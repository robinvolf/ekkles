//! Modul pro parsování dat z formátu, který používá [Opensong](https://opensong.org/development/file-formats/)
//! do formátu používaného Ekklesem.

use crate::data::{PartTag, Song};
use anyhow::{Context, Result, bail};
use roxmltree::Document;
use std::{collections::HashMap, path::Path};

/// Název XML elementu obsahující název písně
const XML_TITLE_ELEM_NAME: &str = "title";
/// Název XML elementu obsahující autora písně
const XML_AUTHOR_ELEM_NAME: &str = "author";
/// Název XML elementu obsahující slova písně
const XML_LYRICS_ELEM_NAME: &str = "lyrics";
/// Název XML elementu obsahující pořadí částí písně
const XML_ORDER_ELEM_NAME: &str = "presentation";

impl Song {
    pub fn parse_from_xml_file(file: &Path) -> Result<Self> {
        todo!()
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
    fn parse_from_xml(xml: &str) -> Result<Self> {
        let document = Document::parse(xml).context("Nelze zparsovat XML")?;

        let title = document
            .descendants()
            .filter(|node| node.is_element())
            .find(|elem| elem.tag_name().name() == XML_TITLE_ELEM_NAME)
            .context("Píseň musí mít název")?
            .text()
            .unwrap() // Unwrap je v pořádku, protože jsme zkontrolovali, že je to element
            .to_string();

        let author = document
            .descendants()
            .filter(|node| node.is_element())
            .find(|elem| elem.tag_name().name() == XML_AUTHOR_ELEM_NAME)
            .and_then(|node| node.text().map(|t| t.to_string()));

        let mut lyrics = document
            .descendants()
            .filter(|node| node.is_element())
            .find(|elem| elem.tag_name().name() == XML_LYRICS_ELEM_NAME)
            .context("Píseň musí obsahovat slova")?
            .text()
            .unwrap() // Unwrap je v pořádku, protože jsme zkontrolovali, že je to element
            .to_string();

        strip_chords_and_newlines(&mut lyrics);
        let split_lyrics_with_tags =
            split_into_parts(&lyrics).context("Chyba rozdělování písně na podčásti")?;

        let order = document
            .descendants()
            .filter(|node| node.is_element())
            .find(|elem| elem.tag_name().name() == XML_ORDER_ELEM_NAME)
            .and_then(|node| node.text())
            .map_or_else(
                // Pokud XML obsahuje údaje o pořadí, využijeme je, jinak použijeme pořadí, jak jsou jednotlivé části zapsané
                || {
                    split_lyrics_with_tags
                        .iter()
                        .map(|(_lyric, tag)| tag.clone())
                        .collect()
                },
                |text| {
                    let order: Vec<String> = text.split(' ').map(|s| s.to_string()).collect();

                    order
                },
            );

        let parts: HashMap<_, _> = split_lyrics_with_tags
            .into_iter()
            .map(|x| (x.1, x.0))
            .collect();

        Ok(Self {
            title,
            author,
            parts,
            order,
        })
    }
}

/// Odstraní akordy a znaky nového řádku ze slov písně. Akordy jsou řádky začínající znakem `'.'` (tečka).
fn strip_chords_and_newlines(lyrics: &mut String) {
    *lyrics = lyrics
        .lines()
        .filter(|&l| {
            if l.len() > 0 && l.chars().nth(0).unwrap() == '.' {
                false
            } else {
                true
            }
        })
        .collect();
}

/// Rozdělí slova do podčástí. Používá k tomu separátory, které vypadají následovně:
/// `[` `tag` `]`, `tag` je libovolný řetězec znaků a je poté použit pro identifikaci dané části.
///
/// ### Návratová hodnota
/// Vrací vektor dvojic `(slova, tag)`, pokud nastane chyba, vrací Error.
///
/// ### Implementace
/// Na toto by se docela hodil regex, ale nechci ho mít v závislostech kvůli něčemu tak jednoduchému.
/// Místo toho je tento proces implementován jednoduchým dvoustavovým konečným automatem (`processing_tag`).
fn split_into_parts(lyrics: &str) -> Result<Vec<(String, PartTag)>> {
    let mut result = Vec::new();
    let mut processing_tag = false;
    let mut current_lyric = String::new();
    let mut current_tag = String::new();

    for ch in lyrics.chars() {
        match ch {
            '[' if processing_tag => {
                bail!("Nesprávný formát tagů, neukončená otevírací závorka \"[\"")
            }
            '[' => {
                processing_tag = true;
                if !current_lyric.is_empty() && !current_tag.is_empty() {
                    // Kontrola na prázdnost, aby nám to na začátek nedalo dvojici prázdných řetězců
                    result.push((
                        current_lyric.trim().to_string(),
                        current_tag.trim().to_string(),
                    ));
                    current_lyric.clear();
                    current_tag.clear();
                }
            }
            ']' if processing_tag => processing_tag = false,
            ']' => bail!("Nesprávný formát tagů, přebytečná uzavírací závorka \"]\""),
            _ if processing_tag => current_tag.push(ch),
            _ => current_lyric.push(ch),
        }
    }

    // Musíme vložit ještě poslední, protože se vkládá vždy až při následujícím tagu (a poslední tag nemá žádný následující)
    result.push((
        current_lyric.trim().to_string(),
        current_tag.trim().to_string(),
    ));

    return Ok(result);
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW_LYRICS: &str = r"[V1]
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
 He tore the bars away, Je____sus my Lord!";

    const CHORDLESS_LYRICS: &str = "[V1] Low in the grave He lay, Jesus my Savior! Waiting the coming day, Je____sus my Lord![C] (Spirited!) Up from the grave He arose, With a mighty triumph o'er His foes; He arose a victor from the dark do_main, And He lives forever with His saints to   reign, He arose! He arose! Hallelujah! Christ arose![V2] Vainly they watch His bed, Jesus my Savior! Vainly they seal the dead, Je____sus my Lord![V3] Death cannot keep his prey, Jesus my Savior! He tore the bars away, Je____sus my Lord!";

    #[test]
    fn strip_chords_test() {
        let mut input = RAW_LYRICS.to_string();
        strip_chords_and_newlines(&mut input);
        assert_eq!(input, CHORDLESS_LYRICS);
    }

    #[test]
    fn split_into_parts_test() {
        let expected = vec![
            (
                String::from(
                    "Low in the grave He lay, Jesus my Savior! Waiting the coming day, Je____sus my Lord!",
                ),
                String::from("V1"),
            ),
            (
                String::from(
                    "(Spirited!) Up from the grave He arose, With a mighty triumph o'er His foes; He arose a victor from the dark do_main, And He lives forever with His saints to   reign, He arose! He arose! Hallelujah! Christ arose!",
                ),
                String::from("C"),
            ),
            (
                String::from(
                    "Vainly they watch His bed, Jesus my Savior! Vainly they seal the dead, Je____sus my Lord!",
                ),
                String::from("V2"),
            ),
            (
                String::from(
                    "Death cannot keep his prey, Jesus my Savior! He tore the bars away, Je____sus my Lord!",
                ),
                String::from("V3"),
            ),
        ];
        let res = split_into_parts(CHORDLESS_LYRICS).expect("Slova jsou ve správném formátu");
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
                        "Low in the grave He lay, Jesus my Savior! Waiting the coming day, Je____sus my Lord!",
                    ),
                ),
                (
                    String::from("C"),
                    String::from(
                        "(Spirited!) Up from the grave He arose, With a mighty triumph o'er His foes; He arose a victor from the dark do_main, And He lives forever with His saints to   reign, He arose! He arose! Hallelujah! Christ arose!",
                    ),
                ),
                (
                    String::from("V2"),
                    String::from(
                        "Vainly they watch His bed, Jesus my Savior! Vainly they seal the dead, Je____sus my Lord!",
                    ),
                ),
                (
                    String::from("V3"),
                    String::from(
                        "Death cannot keep his prey, Jesus my Savior! He tore the bars away, Je____sus my Lord!",
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
                    String::from("Haleluja, haleluja, vládne nám všemocný Bůh a Král."),
                ),
                (
                    String::from("V1a"),
                    String::from(
                        "Haleluja, Svatý, Svatý, Svatý Pán Bůh Všemohoucí, hoden je On sám, Beránek, náš Pán, přijmout chválu,",
                    ),
                ),
                (
                    String::from("V1b"),
                    String::from(
                        "Svatý, Svatý Pán Bůh Všemohoucí, hoden je On sám, Beránek, náš Pán, přijmout chválu.",
                    ),
                ),
                (
                    String::from("V2a"),
                    String::from(
                        "Haleluja, Svatý, Svatý, Ty jsi náš Bůh Všemohoucí, přijmi, Pane náš, přijmi, Pane náš, naši chválu,",
                    ),
                ),
                (
                    String::from("V2b"),
                    String::from(
                        "Svatý, Ty jsi náš Bůh Všemohoucí, přijmi, Pane náš, přijmi, Pane náš, chválu.",
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
