//! Modul pro interakci s databází

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sqlx::{SqlitePool, query_file, sqlite::SqliteConnectOptions};
use tokio::fs::OpenOptions;

/// Připojí se k SQLite databázi na cestě `db_path`, pokud se připojení nezdaří, vrátí Error.
pub async fn open_database(db_path: impl AsRef<Path>) -> Result<SqlitePool> {
    let db_options = SqliteConnectOptions::new()
        .filename(db_path)
        .optimize_on_close(true, None);

    let db_pool = SqlitePool::connect_with(db_options)
        .await
        .context("Nelze se připojit k databázi")?;

    Ok(db_pool)
}

/// Vytvoří novou databázi na cestě `path` a nalije do ní prázdnou databázi Ekklesu.
///
/// - Pokud na cestě `path` existuje nějaký soubor bude přepsán!
pub async fn create_new_database(path: impl AsRef<Path>) -> Result<SqlitePool> {
    // Separátní scope, abychom tady dropli File, tímto ho přepíšeme/vytvoříme
    {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path.as_ref())
            .await
            .with_context(|| {
                format!(
                    "Nelze vytvořit nový soubor pro databázi na {}",
                    path.as_ref().display()
                )
            })?;
    }

    let db = open_database(path.as_ref()).await?;

    query_file!("db/init_db.sql")
        .execute(&db)
        .await
        .context("Nelze inicializovat databázi")?;

    Ok(db)
}

/// Otvře databázi na cestě `path`, pokud neexistuje, bude vytvořena a inicializována.
/// Pokud se na této cestě předtím vyskytoval jiný soubor, bude přepsán.
pub async fn open_or_create_database(path: impl AsRef<Path>) -> Result<SqlitePool> {
    match open_database(path.as_ref()).await {
        Ok(db) => Ok(db),
        Err(_) => create_new_database(path.as_ref()).await.with_context(|| {
            format!(
                "Nelze vytvořit nový soubor pro databázi na {}",
                path.as_ref().display()
            )
        }),
    }
}
