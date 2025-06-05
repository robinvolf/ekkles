//! Modul pro manipulaci s databází, načítání a ukládání dat.

use sqlx::{Error, SqlitePool, sqlite::SqliteConnectOptions};
use std::path::Path;

/// Připojí se k SQLite databázi, která se nachází v souboru `filename` a vrátí connection pool pro danou databázi.
async fn connect(filename: impl AsRef<Path>) -> impl Future<Output = Result<SqlitePool, Error>> {
    let options = SqliteConnectOptions::new()
        .filename(filename)
        .create_if_missing(true);

    SqlitePool::connect_with(options)
}
