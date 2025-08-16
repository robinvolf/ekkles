use std::path::PathBuf;

use anyhow::{Context, Result};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};

/// Připojí se k SQLite databázi na cestě `db_path`, pokud se připojení nezdaří, vrátí Error.
pub async fn connect_db(db_path: PathBuf) -> Result<SqlitePool> {
    let db_options = SqliteConnectOptions::new()
        .filename(db_path)
        .optimize_on_close(true, None);

    let db_pool = SqlitePool::connect_with(db_options)
        .await
        .context("Nelze se připojit k databázi")?;

    Ok(db_pool)
}
