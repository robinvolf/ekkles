use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use ekkles_data::Song;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::path::{Path, PathBuf};

/// Malá utilitka k programu Ekkles, která slouží k importu písní (ve formátu Opensongu) a biblí (ve formátu z github repozitáře) do databáze Ekklesu.
#[derive(Parser, Debug)]
struct Cli {
    /// Co se bude parsovat
    parse_kind: ParseKind,
    /// Soubor obsahující SQLite3 databázi.
    db_file: PathBuf,
    /// Vstupní XML soubory bible nebo písní
    input_files: Vec<PathBuf>,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ParseKind {
    /// Budou se parsovat Bible
    Bible,
    /// Budou se parsovat písně
    Song,
}

async fn connect_to_db(path: &Path) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .optimize_on_close(true, None);

    let pool = SqlitePool::connect_with(options)
        .await
        .context("Nelze se připojit k databázi")?;

    Ok(pool)
}

async fn run(config: Cli) -> Result<()> {
    let db_options = SqliteConnectOptions::new()
        .filename(config.db_file)
        .optimize_on_close(true, None);

    let db_pool = SqlitePool::connect_with(db_options)
        .await
        .context("Nelze se připojit k databázi")?;

    let total = config.input_files.len();
    let mut successes = 0;
    let mut fails = 0;

    for input_file in config.input_files {
        match config.parse_kind {
            ParseKind::Bible => todo!(),
            ParseKind::Song => {
                let res = Song::parse_from_xml_file(&input_file);
                match res {
                    Ok(song) => {
                        song.save_to_db(&db_pool).await.with_context(|| {
                            format!("Nelze uložit píseň {} do databáze", song.title)
                        })?;
                        successes += 1;
                    }
                    Err(err) => {
                        eprintln!(
                            "Nelze zparsovat píseň ze souboru {}: {}",
                            input_file.display(),
                            err
                        );
                        fails += 1;
                    }
                }
                println!("{:04} + {:04} / {:04}", successes, fails, total);
            }
        }
    }

    println!("=== HOTOVO ===");
    println!("Úspěšných = {}, Selhaných = {}", successes, fails);

    Ok(())
}

fn main() -> Result<()> {
    let config = Cli::parse();

    if config.input_files.is_empty() {
        bail!("Nebyly zadány žádné vstupní soubory k parsování, končím");
    }

    eprintln!("Načtena konfigurace: {:#?}", config);

    Ok(())
}
