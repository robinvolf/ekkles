//! Modul pro parsování dat z formátu, který používá [Opensong](https://opensong.org/development/file-formats/)
//! do formátu používaného Ekklesem.
//!
//! ### Výkonnost
//! Tento modul není napsán s ohledem na výkon, spousta klonování `String`ů,
//! kde by se dalo něco znovupoužít. Pokud to bude problém, lze to přepsat,
//! ale jelikož je to pouze pro jednorázový import, mělo by to být v pořádku

use crate::data::{PartTag, Song};
use anyhow::{Context, Result, bail};
use lazy_static::lazy_static;
use regex::{self, Regex, RegexBuilder};
use roxmltree::Document;
use std::{collections::HashMap, fs::read_to_string, path::Path};

/// Název XML elementu obsahující název písně
const XML_TITLE_ELEM_NAME: &str = "title";
/// Název XML elementu obsahující autora písně
const XML_AUTHOR_ELEM_NAME: &str = "author";
/// Název XML elementu obsahující slova písně
const XML_LYRICS_ELEM_NAME: &str = "lyrics";
/// Název XML elementu obsahující pořadí částí písně
const XML_ORDER_ELEM_NAME: &str = "presentation";

lazy_static! {
    /// Matchne řádek (včetně znaku nového řádku) s akordy.
    static ref CHORD_AND_EMPTY_LINES_REGEX: Regex = RegexBuilder::new(r"(^\.[\w/ ]*\n)|(^\s*\n)")
        .multi_line(true)
        .build()
        .unwrap();
    /// Matchne vždy dvojici `[tag]\n slova...`, kde `tag` uloží do capture grupy `tag` a `slova` uloží do capture grupy `part`.
    static ref TAG_VERSE_REGEX: Regex = Regex::new(r"\[(?P<tag>[^\]]+)\]\n(?P<part>[^\[\]]+)(?:\n|$)").unwrap();
}

impl Song {
    /// Zparsuje XML dokument, obsahující píseň, nacházející se v souboru `file`.
    /// Pokud se vše zdaří, vrátí načtenou píseň, jinak vrací Error.
    ///
    /// Více informací o způsobu parsování viz [`Song::parse_from_xml()`]
    pub fn parse_from_xml_file(file: &Path) -> Result<Self> {
        let xml = read_to_string(file)
            .context(format!("Nepodařilo se přečíst soubor {}", file.display()))?;
        let song = Song::parse_from_xml(&xml)
            .context(format!("Nepodařilo se zparsovat soubor {}", file.display()))?;

        Ok(song)
    }

    /// Zparsuje dokument písně `xml` v [XML formátu](https://opensong.org/development/file-formats/).
    /// Pokud lze zparsovat vrátí `Ok(Song)`, jinak `Error`.
    ///
    /// ### Parsování
    /// Vytáhne si z písně:
    /// - Název (povinný, jinak chyba)
    /// - Autor (nepovinný)
    /// - Slova (povinné), ty se posléze zparsují (odstraní se akordy pro kytaru a rozdělí se do příslušných částí - sloka, refrén, ...)
    ///
    /// Pokud je element `presentation` neprázdný, použije se pořadí z něj,
    /// jinak se použije pořadí zapsaných částí písně ve slovech.
    pub fn parse_from_xml(xml: &str) -> Result<Self> {
        let document = Document::parse(xml).context("Nelze zparsovat XML")?;

        let title = document
            .descendants()
            .filter(|node| node.is_element())
            .find(|elem| elem.tag_name().name() == XML_TITLE_ELEM_NAME)
            .context("Píseň musí mít název")?
            .text()
            .context("Název písně je prázdný")?
            .to_string();

        let author = document
            .descendants()
            .filter(|node| node.is_element())
            .find(|elem| elem.tag_name().name() == XML_AUTHOR_ELEM_NAME)
            .and_then(|node| node.text().map(|t| t.to_string()));

        let raw_lyrics = document
            .descendants()
            .filter(|node| node.is_element())
            .find(|elem| elem.tag_name().name() == XML_LYRICS_ELEM_NAME)
            .context("Píseň musí obsahovat slova")?
            .text()
            .context("Slova písně jsou prázdné")?
            .to_string();

        let lyrics = parse_lyrics(&raw_lyrics);
        // Pokud jsou slova prázdné, nemá smysl ukládat píseň
        if lyrics.is_empty() {
            bail!("Nepodařilo se extrahovat slova z písně");
        }

        let order = document
            .descendants()
            .filter(|node| node.is_element())
            .find(|elem| elem.tag_name().name() == XML_ORDER_ELEM_NAME)
            .and_then(|node| node.text())
            .map_or_else(
                // Pokud XML obsahuje údaje o pořadí, využijeme je, jinak použijeme pořadí, jak jsou jednotlivé části zapsané
                || lyrics.iter().map(|(tag, _lyric)| tag.clone()).collect(),
                |text| {
                    let order: Vec<String> = text.split(' ').map(|s| s.to_string()).collect();

                    order
                },
            );

        let parts: HashMap<_, _> = lyrics.into_iter().map(|x| (x.0, x.1)).collect();

        Ok(Self {
            title,
            author,
            parts,
            order,
        })
    }
}

/// Zpracuje slova z jejich surové reprezentace v XML do vektoru dvojic `(tag, část)`.
/// Zachová znaky nového řádku v jednotlivých částí, aby jednotlivé řádky reprezentovaly
/// jednotlivé verše písně.
///
/// ### Výsledek
/// Rozdělování probíhá na základě regulárních výrazů, v případě, že slova neodpovídají
/// danému formátu, bude vrácen prázdný vektor.
///
/// ### Akordy
/// Pokud jsou ve slovech přítomné akordy (řádky začínající `.`), jsou odstraněny.
///
/// ### Rozdělení
/// Rozdělí slova do podčástí. Používá k tomu separátory, které vypadají následovně:
/// `[` `tag` `]`, `tag` je libovolný řetězec znaků a je poté použit pro identifikaci dané části.
fn parse_lyrics(raw_lyrics: &str) -> Vec<(PartTag, String)> {
    // Odstranění whitespace znaků ze začátku a konce každého řádku
    let trimmed = {
        // Toto je strašlivý hack, potřeboval bych mezi každé dvě položky iterátoru strčit '\n',
        // ale https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.intersperse není
        // stabilizované, tudíž to musím strčit za každou položku a potom to odstranit za poslední
        let mut trimmed: String = raw_lyrics
            .lines()
            .map(|line| {
                let mut trimmed_line = line.trim().to_string();
                trimmed_line.push('\n'); // lines() odstraňuje znaky nového řádku, musíme je zpátky přidat
                trimmed_line
            })
            .collect();
        match trimmed.pop() {
            // Pokud jsme narazili na něco jiného než znak nového řádku, vrátíme ho zpátky
            Some(ch) if ch != '\n' => trimmed.push(ch),
            // Pokud je řetězec prázdný (None) nebo byl vrácen znak nového řádku, úspěch
            _ => (),
        }
        trimmed
    };

    // Odstranění řádků s akordy a prázdných řádků
    let chordless_without_empty_lines = CHORD_AND_EMPTY_LINES_REGEX.replace_all(&trimmed, "");

    // Extrakce dvojic (tag, slova)
    TAG_VERSE_REGEX
        .captures_iter(&chordless_without_empty_lines)
        .map(|capture| (capture["tag"].to_string(), capture["part"].to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lyrics_test() {
        const RAW_LYRICS: &str = r"[V1]
 Low in the grave He lay, Jesus my Savior!
.Eb          Bb          Gm/Bb C      F
 Waiting the coming day, Je____sus my Lord!

[C]
.            Bb
 (Spirited!) Up from the grave He arose,
.              Cm               Bb
 With a mighty triumph o'er His foes;
.    F                      Gm   Eb Bb
 He arose a victor from the dark do_main,
.       Eb       C             F      C7/G F/A
 And He lives forever with His saints to   reign,
.    Bb        Eb         Bb     F       Bb
 He arose! He arose! Hallelujah! Christ arose!

[V2]
.Bb                         F        Eb Bb
 Vainly they watch His bed, Jesus my Savior!
.Eb          Bb             Gm/Bb C      F
 Vainly they seal the dead, Je____sus my Lord!

[V3]
.Bb                          F        Eb Bb
 Death cannot keep his prey, Jesus my Savior!
.Eb          Bb         Gm/Bb C      F
 He tore the bars away, Je____sus my Lord!";

        let expected = vec![
            (
                String::from("V1"),
                String::from(
                    "Low in the grave He lay, Jesus my Savior!\nWaiting the coming day, Je____sus my Lord!",
                ),
            ),
            (
                String::from("C"),
                String::from(
                    "(Spirited!) Up from the grave He arose,\nWith a mighty triumph o'er His foes;\nHe arose a victor from the dark do_main,\nAnd He lives forever with His saints to   reign,\nHe arose! He arose! Hallelujah! Christ arose!",
                ),
            ),
            (
                String::from("V2"),
                String::from(
                    "Vainly they watch His bed, Jesus my Savior!\nVainly they seal the dead, Je____sus my Lord!",
                ),
            ),
            (
                String::from("V3"),
                String::from(
                    "Death cannot keep his prey, Jesus my Savior!\nHe tore the bars away, Je____sus my Lord!",
                ),
            ),
        ];
        let res = parse_lyrics(&RAW_LYRICS);
        assert_eq!(res, expected);
    }

    #[test]
    fn parse_from_xml_test() {
        const CHRIST_AROSE_RAW_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<song>
  <title>Christ Arose</title>
  <author>Robert Lowry, 1874</author>
  <copyright>Public Domain</copyright>
  <presentation>V1 C V2 C V3 C</presentation>
  <capo print="true">3</capo>
  <tempo></tempo>
  <timesig></timesig>
  <ccli>27783</ccli>
  <theme>Christ: Victory</theme>
  <alttheme></alttheme>
  <user1></user1>
  <user2></user2>
  <user3></user3>
  <lyrics>[V1]
.Bb                       F        Eb Bb
 Low in the grave He lay, Jesus my Savior!
.Eb          Bb          Gm/Bb C      F
 Waiting the coming day, Je____sus my Lord!

[C]
.            Bb
 (Spirited!) Up from the grave He arose,
.              Cm               Bb
 With a mighty triumph o'er His foes;
.    F                      Gm   Eb Bb
 He arose a victor from the dark do_main,
.       Eb       C             F      C7/G F/A
 And He lives forever with His saints to   reign,
.    Bb        Eb         Bb     F       Bb
 He arose! He arose! Hallelujah! Christ arose!

[V2]
.Bb                         F        Eb Bb
 Vainly they watch His bed, Jesus my Savior!
.Eb          Bb             Gm/Bb C      F
 Vainly they seal the dead, Je____sus my Lord!

[V3]
.Bb                          F        Eb Bb
 Death cannot keep his prey, Jesus my Savior!
.Eb          Bb         Gm/Bb C      F
 He tore the bars away, Je____sus my Lord!</lyrics></song>"#;

        const HALELUJA_RAW_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<song>
  <title>Haleluja (Svatý Pán Bůh Všemohoucí)</title>
  <lyrics>[C]
 Haleluja, haleluja,
 vládne nám všemocný Bůh a Král.

[V1a]
 Haleluja, Svatý, Svatý,
 Svatý Pán Bůh Všemohoucí,
 hoden je On sám,
 Beránek, náš Pán,
 přijmout chválu,

[V1b]
 Svatý, Svatý Pán Bůh Všemohoucí,
 hoden je On sám,
 Beránek, náš Pán,
 přijmout chválu.

[V2a]
 Haleluja, Svatý, Svatý,
 Ty jsi náš Bůh Všemohoucí,
 přijmi, Pane náš,
 přijmi, Pane náš,
 naši chválu,

[V2b]
 Svatý, Ty jsi náš Bůh Všemohoucí,
 přijmi, Pane náš,
 přijmi, Pane náš,
 chválu.</lyrics>
  <author></author>
  <copyright></copyright>
  <hymn_number></hymn_number>
  <presentation></presentation>
  <ccli></ccli>
  <capo print="false"></capo>
  <key></key>
  <aka></aka>
  <key_line></key_line>
  <user1></user1>
  <user2></user2>
  <user3></user3>
  <theme></theme>
  <linked_songs/>
  <tempo></tempo>
  <time_sig></time_sig>
  <backgrounds resize="screen" keep_aspect="false" link="false" background_as_text="false"/>
</song>"#;

        let christ_arose_expected = Song {
            title: String::from("Christ Arose"),
            author: Some(String::from("Robert Lowry, 1874")),
            parts: HashMap::from([
                (
                    String::from("V1"),
                    String::from(
                        "Low in the grave He lay, Jesus my Savior!\nWaiting the coming day, Je____sus my Lord!",
                    ),
                ),
                (
                    String::from("C"),
                    String::from(
                        "(Spirited!) Up from the grave He arose,\nWith a mighty triumph o'er His foes;\nHe arose a victor from the dark do_main,\nAnd He lives forever with His saints to   reign,\nHe arose! He arose! Hallelujah! Christ arose!",
                    ),
                ),
                (
                    String::from("V2"),
                    String::from(
                        "Vainly they watch His bed, Jesus my Savior!\nVainly they seal the dead, Je____sus my Lord!",
                    ),
                ),
                (
                    String::from("V3"),
                    String::from(
                        "Death cannot keep his prey, Jesus my Savior!\nHe tore the bars away, Je____sus my Lord!",
                    ),
                ),
            ]),
            order: vec![
                String::from("V1"),
                String::from("C"),
                String::from("V2"),
                String::from("C"),
                String::from("V3"),
                String::from("C"),
            ],
        };

        let haleluja_expected = Song {
            title: String::from("Haleluja (Svatý Pán Bůh Všemohoucí)"),
            author: None,
            parts: HashMap::from([
                (
                    String::from("C"),
                    String::from("Haleluja, haleluja,\nvládne nám všemocný Bůh a Král."),
                ),
                (
                    String::from("V1a"),
                    String::from(
                        "Haleluja, Svatý, Svatý,\nSvatý Pán Bůh Všemohoucí,\nhoden je On sám,\nBeránek, náš Pán,\npřijmout chválu,",
                    ),
                ),
                (
                    String::from("V1b"),
                    String::from(
                        "Svatý, Svatý Pán Bůh Všemohoucí,\nhoden je On sám,\nBeránek, náš Pán,\npřijmout chválu.",
                    ),
                ),
                (
                    String::from("V2a"),
                    String::from(
                        "Haleluja, Svatý, Svatý,\nTy jsi náš Bůh Všemohoucí,\npřijmi, Pane náš,\npřijmi, Pane náš,\nnaši chválu,",
                    ),
                ),
                (
                    String::from("V2b"),
                    String::from(
                        "Svatý, Ty jsi náš Bůh Všemohoucí,\npřijmi, Pane náš,\npřijmi, Pane náš,\nchválu.",
                    ),
                ),
            ]),
            order: vec![
                String::from("C"),
                String::from("V1a"),
                String::from("V1b"),
                String::from("V2a"),
                String::from("V2b"),
            ],
        };

        let christ_arose_result =
            Song::parse_from_xml(CHRIST_AROSE_RAW_XML).expect("Píseň je ve správném formátu");
        let haleluja_result =
            Song::parse_from_xml(HALELUJA_RAW_XML).expect("Píseň je ve správném formátu");

        assert_eq!(christ_arose_result, christ_arose_expected);
        assert_eq!(haleluja_result, haleluja_expected);
    }
}
