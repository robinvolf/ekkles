//! Modul pro interakci s databází

use std::{fs::create_dir_all, path::Path};

use anyhow::{Context, Result};
use sqlx::{SqlitePool, query, sqlite::SqliteConnectOptions};
use tokio::fs::{DirBuilder, OpenOptions};

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
        let new_database_directory = path
            .as_ref()
            .parent()
            .context("Nelze získat cestu ke složce s databází")?;
        DirBuilder::new()
            .recursive(true)
            .create(new_database_directory)
            .await
            .with_context(|| {
                format!(
                    "Nelze vytvořit složku pro databázi {}",
                    new_database_directory.display()
                )
            })?;
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

    query!("
        DROP TABLE IF EXISTS songs;
        DROP TABLE IF EXISTS song_parts;
        DROP TABLE IF EXISTS translations;
        DROP TABLE IF EXISTS books;
        DROP TABLE IF EXISTS verses;
        DROP TABLE IF EXISTS playlists;
        DROP TABLE IF EXISTS playlist_parts;
        DROP TABLE IF EXISTS playlist_songs;
        DROP TABLE IF EXISTS playlist_passages;

        CREATE TABLE IF NOT EXISTS songs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL UNIQUE,
            author TEXT,
            part_order TEXT NOT NULL -- Vektor uložený jako text, trochu hack
        );

        CREATE TABLE IF NOT EXISTS song_parts (
            song_id INTEGER NOT NULL,
            tag TEXT NOT NULL,
            lyrics TEXT NOT NULL,
            PRIMARY KEY (song_id, tag),
            FOREIGN KEY (song_id) REFERENCES songs (id) ON DELETE CASCADE -- Při smazání písně budou automaticky smazány všechny její části
        );

        CREATE TABLE IF NOT EXISTS translations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS books (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            book_order INTEGER NOT NULL UNIQUE, -- Pořadí knih v Bible (Genesis, Exodus, ... Zjevení)
            title TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS verses (
            translation_id INTEGER NOT NULL,
            book_id INTEGER NOT NULL,
            chapter INTEGER NOT NULL,
            number INTEGER NOT NULL,
            content TEXT NOT NULL,
            -- Pořadí veršů v daném překladu, abychom se mohli jednoduše dotazovat na rozsahy
            verse_order INTEGER NOT NULL,
            PRIMARY KEY (translation_id, book_id, chapter, number),
            FOREIGN KEY (book_id) REFERENCES books (id),
            FOREIGN KEY (translation_id) REFERENCES translations (id)
        );

        CREATE TABLE IF NOT EXISTS playlists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            -- Kdy byl playlist vytvořen, může být použito pro řazení playlistů
            created TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        -- playlist_part může být buď pasáž z Bible nebo píseň (v budoucnu možná další),
        -- vytvoříme tedy pro každou možnost separátní tabulku, ze které se budeme odkazovat
        -- na PK tabulky `playlist_parts`
        CREATE TABLE IF NOT EXISTS playlist_parts (
            playlist_id INTEGER NOT NULL,
            part_order INTEGER NOT NULL,
            kind TEXT NOT NULL CHECK (kind IN ('song', 'bible')),
            PRIMARY KEY (playlist_id, part_order),
            FOREIGN KEY (playlist_id) REFERENCES playlists (id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS playlist_songs (
            playlist_id INTEGER NOT NULL,
            part_order INTEGER NOT NULL,
            song_id INTEGER NOT NULL,
            PRIMARY KEY (playlist_id, part_order),
            FOREIGN KEY (playlist_id) REFERENCES playlists (id) ON DELETE CASCADE,
            FOREIGN KEY (song_id) REFERENCES songs (id)
        );

        CREATE TABLE IF NOT EXISTS playlist_passages (
            playlist_id INTEGER NOT NULL,
            part_order INTEGER NOT NULL,
            translation_id INTEGER NOT NULL,
            start_book_id INTEGER NOT NULL,
            start_chapter INTEGER NOT NULL,
            start_number INTEGER NOT NULL,
            end_book_id INTEGER NOT NULL,
            end_chapter INTEGER NOT NULL,
            end_number INTEGER NOT NULL,
            PRIMARY KEY (playlist_id, part_order),
            FOREIGN KEY (playlist_id) REFERENCES playlists (id) ON DELETE CASCADE,
            FOREIGN KEY (translation_id, start_book_id, start_chapter, start_number) REFERENCES verses (translation_id, book_id, chapter, number),
            FOREIGN KEY (translation_id, end_book_id, end_chapter, end_number) REFERENCES verses (translation_id, book_id, chapter, number)
        );

        INSERT INTO books (id, book_order, title) VALUES
            (0, 0, 'Genesis'),
            (1, 1, 'Exodus'),
            (2, 2, 'Leviticus'),
            (3, 3, 'Numeri'),
            (4, 4, 'Deuteronomium'),
            (5, 5, 'Jozue'),
            (6, 6, 'Soudců'),
            (7, 7, 'Rút'),
            (8, 8, '1. Samuelova'),
            (9, 9, '2. Samuelova'),
            (10, 10, '1. Královská'),
            (11, 11, '2. Královská'),
            (12, 12, '1. Paralipomenon'),
            (13, 13, '2. Paralipomenon'),
            (14, 14, 'Ezdráš'),
            (15, 15, 'Nehemjáš'),
            (16, 16, 'Ester'),
            (17, 17, 'Jób'),
            (18, 18, 'Žalmy'),
            (19, 19, 'Přísloví'),
            (20, 20, 'Kazatel'),
            (21, 21, 'Píseň písní'),
            (22, 22, 'Izajáš'),
            (23, 23, 'Jeremjáš'),
            (24, 24, 'Pláč'),
            (25, 25, 'Ezechiel'),
            (26, 26, 'Daniel'),
            (27, 27, 'Ozeáš'),
            (28, 28, 'Jóel'),
            (29, 29, 'Ámos'),
            (30, 30, 'Abdijáš'),
            (31, 31, 'Jonáš'),
            (32, 32, 'Micheáš'),
            (33, 33, 'Nahum'),
            (34, 34, 'Abakuk'),
            (35, 35, 'Sofonjáš'),
            (36, 36, 'Ageus'),
            (37, 37, 'Zacharjáš'),
            (38, 38, 'Malachiáš'),
            (39, 39, 'Matouš'),
            (40, 40, 'Marek'),
            (41, 41, 'Lukáš'),
            (42, 42, 'Jan'),
            (43, 43, 'Skutky'),
            (44, 44, 'Římanům'),
            (45, 45, '1. Korintským'),
            (46, 46, '2. Korintským'),
            (47, 47, 'Galatským'),
            (48, 48, 'Efezským'),
            (49, 49, 'Filipským'),
            (50, 50, 'Koloským'),
            (51, 51, '1. Tesalonickým'),
            (52, 52, '2. Tesalonickým'),
            (53, 53, '1. Timoteovi'),
            (54, 54, '2. Timoteovi'),
            (55, 55, 'Titovi'),
            (56, 56, 'Filemonovi'),
            (57, 57, 'Židům'),
            (58, 58, 'Jakub'),
            (59, 59, '1. Petrova'),
            (60, 60, '2. Petrova'),
            (61, 61, '1. Janova'),
            (62, 62, '2. Janova'),
            (63, 63, '3. Janova'),
            (64, 64, 'Juda'),
            (65, 65, 'Zjevení');
        ")
        .execute(&db)
        .await
        .context("Nelze inicializovat databázi")?;

    Ok(db)
    // todo!()
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
