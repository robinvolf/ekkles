use std::{env, path::PathBuf};

use const_format::{Case, formatcp, map_ascii_case};

use crate::PROGRAM_NAME;

const DATABASE_NAME: &str = "database.sqlite3";
const DEFAULT_USER_DATA_DIR: &str = ".local/share";
const DB_PATH_ENV: &str = formatcp!("{}_DB_PATH", map_ascii_case!(Case::Upper, PROGRAM_NAME));

/// Konfigurace Ekklesu
#[derive(Debug)]
pub struct Config {
    /// Cesta k databázi s daty
    pub db_path: PathBuf,
}

impl Config {
    pub fn new() -> Self {
        Self { db_path: db_path() }
    }
}

/// Vrátí cestu k databázi, nalezne ji následujícím způsobem:
/// - Podle proměnné prostředí EKKLES_DB_PATH
/// - Složka pro uživatelská data
///   - Podle $XDG_DATA_HOME a pokud je prázdná, tak ~/.local/share
/// - V ní se vytvoří (pokud neexistuje složka) s názvem programu [`crate::PROGRAM_NAME`]
/// - V ní se vybere soubor [`DATABASE_NAME`]
fn db_path() -> PathBuf {
    if let Ok(path) = env::var(DB_PATH_ENV) {
        return path.into();
    };

    if cfg!(debug_assertions) {
        panic!(
            "Během vývoje nebudu modifikovat domovskou složku, nastav si proměnnou {DB_PATH_ENV} na cestu k vývojové databázi"
        );
    }

    let user_data_directory = match env::var("XDG_DATA_HOME") {
        Ok(s) => PathBuf::from(s),
        Err(_) => {
            let home_dir =
                PathBuf::from(env::var("HOME").expect("Proměnná prostředí HOME není definovaná"));
            home_dir.join(DEFAULT_USER_DATA_DIR)
        }
    };

    let program_data_directory = user_data_directory.join(PROGRAM_NAME);

    let db_path = program_data_directory.join(DATABASE_NAME);

    db_path
}
