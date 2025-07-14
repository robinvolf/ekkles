use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use ekkles_data::Song;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::path::PathBuf;

/// Malá utilitka k programu Ekkles, která slouží k importu písní (ve formátu Opensongu)
/// a biblí (ve formátu z github repozitáře) do databáze Ekklesu.
#[derive(Parser, Debug)]
struct Cli {
    /// Co se bude parsovat
    parse_kind: ParseKind,
    /// Soubor obsahující SQLite3 databázi.
    db_file: PathBuf,
    /// Vstupní XML soubory bible nebo písní
    input_files: Vec<PathBuf>,
    /// Určuje, jak nakládat s biblemi/písněmi, které již v databázi existují.
    /// Ve výchozím nastavení jsou takové vstupy ignorovány (v databázi jsou zachována
    /// původní data), pokud je specifikována tato vlaječka, budou namísto toho
    /// existující záznamy přepsány.
    #[arg(long, short)]
    overwrite_records: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ParseKind {
    /// Budou se parsovat Bible
    Bible,
    /// Budou se parsovat písně
    Song,
}

/// Hlavní funkce programu, cyklus postupně načítá všechny soubory specifikované
/// na příkazové řádce (`config`), každý se pokusí zparsovat a uložit do databáze.
///
/// ### Přepis existujícího záznamu
/// Jestli se přepisuje záleží na konfiguraci (viz [`Cli`]).
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
    println!("Úspěch + Selhání / Celkem");
    for input_file in config.input_files {
        match config.parse_kind {
            ParseKind::Bible => todo!(),
            ParseKind::Song => {
                let res = Song::parse_from_xml_file(&input_file);
                match res {
                    Ok(song) => {
                        if config.overwrite_records {
                            todo!()
                        } else {
                            match song.save_to_db(&db_pool).await {
                                Ok(_) => successes += 1,
                                Err(err) => {
                                    eprintln!("{:?}", err);
                                    fails += 1;
                                }
                            };
                        }
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
                println!("{:04}   + {:04}    / {:04}", successes, fails, total);
            }
        }
    }

    println!("=== HOTOVO ===");
    println!("Úspěšných = {}, Selhaných = {}", successes, fails);

    Ok(())
}

// Spustí jednovláknový runtime, na prostý import písní nepotřebujeme spouštět vícevláknovou aplikaci
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let config = Cli::parse();

    if config.input_files.is_empty() {
        bail!("Nebyly zadány žádné vstupní soubory k parsování, končím");
    }

    run(config).await
}
