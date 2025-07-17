//! Modul pro parsování Bible v XML formátu
//! z [tohoto repa](https://github.com/Beblia/Holy-Bible-XML-Format/tree/master)
//! a ukládání do lokální SQLite databáze.

use anyhow::{Context, Result, bail};
use roxmltree::{Document, Node, TextPos};
use sqlx::{SqlitePool, query};

pub mod indexing;

const XML_TRANSLATION_NAME_ATTRIBUTE: &str = "translation";
const XML_TRANSLATION_NAME_ATTRIBUTE_SECONDARY: &str = "name";
const XML_BOOK_NUMBER_ATTRIBUTE: &str = "number";
const XML_CHAPTER_NUMBER_ATTRIBUTE: &str = "number";
const XML_VERSE_NUMBER_ATTRIBUTE: &str = "number";
const XML_BOOK_TAG_NAME: &str = "book";
const XML_TESTAMENT_TAG_NAME: &str = "testament";
const XML_CHAPTER_TAG_NAME: &str = "chapter";
const XML_VERSE_TAG_NAME: &str = "verse";
/// Je to opravdu konstanta 😎
const NUM_BOOKS_IN_THE_BIBLE: usize = 66;

/// Zparsuje XML bible a uloží ji do databáze pomocí dodaného poolu,
/// v případě chyby vrátí Error.
///
/// ### Transakce
/// Používá mechanismus transakcí, tedy buď kompletně celá kniha bude uložena
/// do databáze nebo ani část z ní (v případě chyby).
///
/// ### Implementace
/// Parsuje formát z [tohoto repa](https://github.com/Beblia/Holy-Bible-XML-Format/tree/master).
/// Nejdřív uloží nový název překladu do databáze a poté začne ukládat jednotlivé verše.
pub async fn parse_bible_from_xml(xml: &str, pool: &SqlitePool) -> Result<()> {
    let document = Document::parse(xml).context("Nelze zparsovat XML")?;

    // Používáme transakci, abychom mohli na konci po úspěšném zparsování spustit `commit()`,
    // jinak je při dropu transakce zrušena (proveden rollback)
    let mut transaction = pool
        .begin()
        .await
        .context("Nelze získat připojení k databázi z poolu")?;

    let translation_name = document
        .root_element()
        .attribute(XML_TRANSLATION_NAME_ATTRIBUTE)
        .or_else(|| {
            document
                .root_element()
                .attribute(XML_TRANSLATION_NAME_ATTRIBUTE_SECONDARY)
        })
        .context("V Dokumentu chybí atribut názvu překladu")?;

    let translation_id = query!(
        "
        INSERT INTO translations (name) VALUES ($1);
        ",
        translation_name
    )
    .execute(&mut *transaction)
    .await
    .context("Nelze uložit název překladu do databáze")?
    .last_insert_rowid();

    // Pozor, tady se musí provést filtrování, protože mezi jednotlivými
    // books/chapters/verses se mohou vyskytovat uzly s textem obsahující pouze whitespace-znaky
    let books = document
        .root_element()
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == XML_TESTAMENT_TAG_NAME)
        .flat_map(|testament| {
            testament
                .children()
                .filter(|node| node.is_element() && node.tag_name().name() == XML_BOOK_TAG_NAME)
        });

    let count = books.clone().count();
    if count != NUM_BOOKS_IN_THE_BIBLE {
        bail!("Nesprávný počet knih ({count})");
    }

    // Closure pro spočítání řádku a sloupce XML uzlu v případě chyby
    let get_pos = |node: Node| -> TextPos {
        let start_byte = node.range().start;
        document.text_pos_at(start_byte)
    };

    let mut verse_order = 0;

    for book in books {
        let book_number = book
            .attribute(XML_BOOK_NUMBER_ATTRIBUTE)
            .with_context(|| {
                format!(
                    "Nelze najít atribut 'number' knihy, na pozici: {}",
                    get_pos(book)
                )
            })?
            .parse::<u32>()
            .with_context(|| {
                format!(
                    "Atribut number je v nesprávném formátu, na pozici: {}",
                    get_pos(book)
                )
            })?;

        let order = book_number_to_order(book_number);

        let book_id = query!("SELECT (id) FROM books WHERE book_order = $1", order)
            .fetch_one(&mut *transaction)
            .await
            .context("Nelze získat id knihy z databáze")?
            .id
            .with_context(|| format!("Kniha s pořadím '{}' v databázi neexistuje", order))?;

        for chapter in book
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == XML_CHAPTER_TAG_NAME)
        {
            let chapter_number = chapter
                .attribute(XML_CHAPTER_NUMBER_ATTRIBUTE)
                .with_context(|| {
                    format!(
                        "Nelze najít atribut 'number' kapitoly, na pozici {}",
                        get_pos(chapter)
                    )
                })?
                .parse::<u32>()
                .with_context(|| {
                    format!(
                        "Atribut number je v nesprávném formátu, na pozici {}",
                        get_pos(chapter)
                    )
                })?;

            for verse in chapter
                .children()
                .filter(|node| node.is_element() && node.tag_name().name() == XML_VERSE_TAG_NAME)
            {
                let verse_number = verse
                    .attribute(XML_VERSE_NUMBER_ATTRIBUTE)
                    .with_context(|| {
                        format!(
                            "Nelze najít atribut 'number' verše, na pozici {}",
                            get_pos(verse)
                        )
                    })?
                    .parse::<u32>()
                    .with_context(|| {
                        format!(
                            "Atribut number je v nesprávném formátu, na pozici {}",
                            get_pos(verse)
                        )
                    })?;

                let verse_content = verse.text().with_context(|| {
                    format!("Verš neobsahuje text na pozici {}", get_pos(verse))
                })?;

                query!(
                        "
                        INSERT INTO verses (translation_id, book_id, chapter, number, content, verse_order) VALUES ($1, $2, $3, $4, $5, $6);
                        ",
                        translation_id,
                        book_id,
                        chapter_number,
                        verse_number,
                        verse_content,
                        verse_order,
                    )
                    .execute(&mut *transaction)
                    .await
                    .context("Nelze uložit verš")?;

                verse_order += 1;
            }
        }
    }

    // Pokud jsme se dostali až sem, znamená to, že nenastala chyba, můžeme commitnout transakci
    transaction
        .commit()
        .await
        .context("Nelze provést commit transakce")?;

    Ok(())
}

/// Převede číslo knihy v XML na tradiční pořadí. V pořadí indexujeme od 0,
/// ale čísla knih jsou od 1.
fn book_number_to_order(number: u32) -> u32 {
    number - 1
}
