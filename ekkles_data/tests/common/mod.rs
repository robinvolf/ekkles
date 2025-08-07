use std::collections::HashMap;

use ekkles_data::Song;
use ekkles_data::bible::parse_bible_from_xml;
use sqlx::SqlitePool;
use sqlx::query_file;
use tokio::fs::read_to_string;

/// Funkce na vytvoření in-memory databáze pro testování. Vytvoří holou databázi
/// pouze se strukturou tabulek, ale bez dat.
pub async fn setup_bare_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    query_file!("db/init_db.sql").execute(&pool).await.unwrap();

    pool
}

/// Funkce na vytvoření in-memory databáze pro testování. Vytvoří databázi
/// a přidá do ní bibli pro testování.
pub async fn setup_db_with_bible() -> SqlitePool {
    let pool = setup_bare_db().await;

    let xml_data = read_to_string("tests/data/CzechPrekladBible.xml")
        .await
        .unwrap();

    parse_bible_from_xml(&xml_data, &pool).await.unwrap();

    pool
}

/// Funkce na vytvoření in-memory databáze pro testování. Vytvoří databázi
/// a přidá do ní bibli (translation_id 0) pro testování + 2 písně (id 0 a 1).
pub async fn setup_db_with_bible_and_songs() -> SqlitePool {
    let pool = setup_db_with_bible().await;

    let haleluja = Song {
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

    let christ_arose = Song {
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

    haleluja.save_to_db(&pool).await.unwrap();
    christ_arose.save_to_db(&pool).await.unwrap();

    pool
}
