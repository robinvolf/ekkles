use ekkles_data::bible::parse_bible_from_xml;
use sqlx::SqlitePool;
use sqlx::query_file;
use tokio::fs::read_to_string;

// Funkce na vytvoření in-memory databáze pro testování. Vytvoří holou databázi
// pouze se strukturou tabulek, ale bez dat.
pub async fn setup_bare_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    query_file!("../db/init_db.sql")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

// Funkce na vytvoření in-memory databáze pro testování. Vytvoří databázi
// a přidá do ní bibli pro testování.
pub async fn setup_db_with_bible() -> SqlitePool {
    let pool = setup_bare_db().await;

    let xml_data = read_to_string("tests/data/CzechPrekladBible.xml")
        .await
        .unwrap();

    parse_bible_from_xml(&xml_data, &pool).await.unwrap();

    pool
}
