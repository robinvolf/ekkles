//! Modul pro parsování Bible v XML formátu
//! z [tohoto repa](https://github.com/Beblia/Holy-Bible-XML-Format/tree/master)
//! a ukládání do lokální SQLite databáze.

use anyhow::{Context, Result, anyhow};
use roxmltree::{Document, Node, TextPos};
use sqlx::{SqlitePool, query};

const XML_TRANSLATION_NAME_ATTRIBUTE: &str = "translation";
const XML_BOOK_NUMBER_ATTRIBUTE: &str = "number";
const XML_CHAPTER_NUMBER_ATTRIBUTE: &str = "number";
const XML_VERSE_NUMBER_ATTRIBUTE: &str = "number";

/// Zparsuje XML bible a uloží ji do databáze pomocí dodaného poolu,
/// v případě chyby vrátí Error.
async fn parse_bible_from_xml(xml: &str, pool: &SqlitePool) -> Result<()> {
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
        .context("V Dokumentu chybí atribut názvu překladu")?;

    query!(
        "
        INSERT OR IGNORE INTO translations (name) VALUES ($1);
        ",
        translation_name
    )
    .execute(&mut *transaction)
    .await
    .context("Nelze uložit název překladu do databáze")?;

    let translation_id = query!(
        "SELECT (id) FROM translations WHERE name = $1",
        translation_name
    )
    .fetch_one(&mut *transaction)
    .await
    .context("Nelze získat id překladu z databáze")?
    .id
    .expect("Překlad s daným názvem byl právě vložen do databáze, musí tam být");

    let old_testament = document
        .root_element()
        .first_child()
        .filter(|node| node.is_element() && node.tag_name().name() == "testament")
        .context("Nelze najít Starý Zákon v XML")?;

    let new_testament = document
        .root_element()
        .last_child()
        .filter(|node| node.is_element() && node.tag_name().name() == "testament")
        .context("Nelze najít Nový Zákon v XML")?;

    let books = old_testament.children().chain(new_testament.children());

    // Closure pro spočítání řádku a sloupce XML uzlu v případě chyby
    let get_pos = |node: Node| -> TextPos {
        let start_byte = node.range().start;
        document.text_pos_at(start_byte)
    };

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

        let (order, book_name) = book_number_to_order_and_name(book_number)
            .with_context(|| format!("Nelze zpracovat číslo knihy, na pozici {}", get_pos(book)))?;
        query!(
            "
            INSERT OR IGNORE INTO books (book_order, title) VALUES ($1, $2);
            ",
            order,
            book_name,
        )
        .execute(&mut *transaction)
        .await
        .context("Nelze uložit knihu do databáze")?;

        let book_id = query!("SELECT (id) FROM books WHERE title = $1", book_name)
            .fetch_one(&mut *transaction)
            .await
            .context("Nelze získat id knihy z databáze")?
            .id
            .expect("Kniha s daným názvem byla právě vložena do databáze, musí tam být");

        for chapter in book.children() {
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

            for verse in chapter.children() {
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
                        INSERT INTO verses (translation_id, book_id, chapter, number, content) VALUES ($1, $2, $3, $4, $5);
                        ",
                        translation_id,
                        book_id,
                        chapter_number,
                        verse_number,
                        verse_content,
                    )
                    .execute(&mut *transaction)
                    .await
                    .context("Nelze uložit verš")?;
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

/// Převede číslo knihy v XML na jméno a tradiční pořadí. Pokud je mimo rozsah (> 66), vrátí Error.
fn book_number_to_order_and_name(number: u32) -> Result<(u32, &'static str)> {
    match number {
        1 => Ok((0, "Genesis")),
        2 => Ok((1, "Exodus")),
        3 => Ok((2, "Leviticus")),
        4 => Ok((3, "Numeri")),
        5 => Ok((4, "Deuteronomium")),
        6 => Ok((5, "Jozue")),
        7 => Ok((6, "Soudců")),
        8 => Ok((7, "Rút")),
        9 => Ok((8, "1. Samuelova")),
        10 => Ok((9, "2. Samuelova")),
        11 => Ok((10, "1. Královská")),
        12 => Ok((11, "2. Královská")),
        13 => Ok((12, "1. Paralipomenon")),
        14 => Ok((13, "2. Paralipomenon")),
        15 => Ok((14, "Ezdráš")),
        16 => Ok((15, "Nehemjáš")),
        17 => Ok((16, "Ester")),
        18 => Ok((17, "Jób")),
        19 => Ok((18, "Žalmy")),
        20 => Ok((19, "Přísloví")),
        21 => Ok((20, "Kazatel")),
        22 => Ok((21, "Píseň písní")),
        23 => Ok((22, "Izajáš")),
        24 => Ok((23, "Jeremjáš")),
        25 => Ok((24, "Pláč")),
        26 => Ok((25, "Ezechiel")),
        27 => Ok((26, "Daniel")),
        28 => Ok((27, "Ozeáš")),
        29 => Ok((28, "Jóel")),
        30 => Ok((29, "Ámos")),
        31 => Ok((30, "Abdijáš")),
        32 => Ok((31, "Jonáš")),
        33 => Ok((32, "Micheáš")),
        34 => Ok((33, "Nahum")),
        35 => Ok((34, "Abakuk")),
        36 => Ok((35, "Sofonjáš")),
        37 => Ok((36, "Ageus")),
        38 => Ok((37, "Zacharjáš")),
        39 => Ok((38, "Malachiáš")),
        40 => Ok((39, "Matouš")),
        41 => Ok((40, "Marek")),
        42 => Ok((41, "Lukáš")),
        43 => Ok((42, "Jan")),
        44 => Ok((43, "Skutky")),
        45 => Ok((44, "Římanům")),
        46 => Ok((45, "1. Korintským")),
        47 => Ok((46, "2. Korintským")),
        48 => Ok((47, "Galatským")),
        49 => Ok((48, "Efezským")),
        50 => Ok((49, "Filipským")),
        51 => Ok((50, "Koloským")),
        52 => Ok((51, "1. Tesalonickým")),
        53 => Ok((52, "2. Tesalonickým")),
        54 => Ok((53, "1. Timoteovi")),
        55 => Ok((54, "2. Timoteovi")),
        56 => Ok((55, "Titovi")),
        57 => Ok((56, "Filemonovi")),
        58 => Ok((57, "Židům")),
        59 => Ok((58, "Jakub")),
        60 => Ok((59, "1. Petrova")),
        61 => Ok((60, "2. Petrova")),
        62 => Ok((61, "1. Janova")),
        63 => Ok((62, "2. Janova")),
        64 => Ok((63, "3. Janova")),
        65 => Ok((64, "Juda")),
        66 => Ok((65, "Zjevení")),
        _ => Err(anyhow!("Nevalidní číslo knihy")),
    }
}
