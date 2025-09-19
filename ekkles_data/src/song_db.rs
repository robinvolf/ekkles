//! Modul pro manipulaci s databází, načítání a ukládání dat.

use std::collections::HashMap;

use crate::Song;
use anyhow::{Context, Result};
use futures::TryStreamExt;
use sqlx::{Sqlite, SqliteConnection, SqlitePool, pool::PoolConnection, query};

const TAG_SPLIT_STRING: &str = " ";

impl Song {
    /// Uloží danou píseň do lokální SQlite databáze, ke které se připojí pomocí `pool`.
    ///
    /// ### Návratová hodnota
    /// - Pokud vše půjde hladce vrací id uložené písně
    /// - V případě chyby vrací Error s jejím popisem
    ///
    /// ### Ošetření chyb
    /// Chyba nastane pokud:
    /// - Píseň není validní (tag v pořadí, který se nevyskytuje ve slovech)
    /// - Píseň nebo její slova nesplňují integritní omezení databáze
    ///
    /// Pokud během ukládání písně do databáze nastane chyba, je proveden rollback celé písně.
    /// Tedy po chybě by databáze měla být ve stejném stavu jako před zavoláním této funkce.
    pub async fn save_to_db(&self, pool: &SqlitePool) -> Result<i64> {
        self.check_invariants()
            .context("Nelze uložit nevalidní píseň")?;

        let mut transaction = pool
            .begin()
            .await
            .context("Nelze získat připojení k databázi z poolu")?;

        let part_order = self.order.join(TAG_SPLIT_STRING);

        let song_id = query!(
            "
            INSERT INTO songs (title, author, part_order) VALUES ($1, $2, $3)
            ",
            self.title,
            self.author,
            part_order
        )
        .execute(&mut *transaction)
        .await
        .context(format!("Nelze uložit píseň {} do databáze", self.title))?
        .last_insert_rowid();

        // TODO: Toto by šlo přepsat, abych místo sekvenčního ukládání spojil všechny query
        // do jedné future pomocí `join_all` a na tom awaitnout
        for (tag, lyrics) in self.parts.iter() {
            query!(
                "INSERT INTO song_parts (song_id, tag, lyrics) VALUES ($1, $2, $3)",
                song_id,
                tag,
                lyrics
            )
            .execute(&mut *transaction)
            .await
            .with_context(|| format!("Nelze uložit část {} písně {}", tag, self.title))?;
        }

        transaction
            .commit()
            .await
            .context("Nelze provést COMMIT uložení písně")?;

        Ok(song_id)
    }

    /// Pokud píseň s názvem `title` v databázi existuje, vrátí její `id`, pokud se
    /// vystkytne při přístupu do databáze chyba nebo daná píseň neexistuje, vrátí Error.
    pub async fn exists_in_db(title: &str, pool: &SqlitePool) -> Result<i64> {
        query!("SELECT id FROM songs WHERE title = $1", title)
            .fetch_one(pool)
            .await
            .with_context(|| format!("Píseň s názvem '{}' nebyla nalezena", title))
            .map(|record| record.id.unwrap())
    }

    /// Smaže píseň s daným `id` z databáze, pokud nastane problém vrátí Error.
    pub async fn delete_from_db(id: i64, pool: &SqlitePool) -> Result<()> {
        query!("DELETE FROM songs WHERE id = $1", id)
            .execute(pool)
            .await
            .with_context(|| format!("Nelze smazat píseň s id {} z databáze", id))?;

        Ok(())
    }

    /// Načte píseň s `id` z SQLite databáze pomocí `conn`.
    ///
    /// ### Ošetření chyb
    /// Vrátí Error, když:
    /// - Se vyskytnou chyby při čtení z databáze
    /// - Načtená píseň nesplňuje invariant (viz dokumentace [Song])
    pub async fn load_from_db(id: i64, conn: &mut PoolConnection<Sqlite>) -> Result<Self> {
        let record = query!(
            "SELECT title, author, part_order FROM songs WHERE id = $1",
            id
        )
        .fetch_one(conn.as_mut())
        .await
        .with_context(|| format!("Píseň s id {id} nebyla nalezena"))?;

        let title = record.title;
        let author = record.author;
        let order: Vec<String> = record
            .part_order
            .split(TAG_SPLIT_STRING)
            .map(|str| str.to_string())
            .collect();

        let mut lyrics = query!("SELECT tag, lyrics FROM song_parts WHERE song_id = $1", id)
            .fetch(conn.as_mut());

        let mut parts = HashMap::new();

        while let Some(record) = lyrics
            .try_next()
            .await
            .context("Nelze načíst část písně z databáze")?
        {
            parts.insert(record.tag, record.lyrics);
        }

        let song = Self {
            title,
            author,
            parts,
            order,
        };

        song.check_invariants().map(|_| song)
    }

    /// Získá vektor dvojic (id, název) všech dostupných písní v databázi. Pokud se vyskytne
    /// při čtení chyba, vrací `Error`.
    pub async fn get_available_from_db(
        conn: &mut PoolConnection<Sqlite>,
    ) -> Result<Vec<(i64, String)>> {
        query!("SELECT id, title FROM songs")
            .map(|record| {
                (
                    record.id.expect("Id je primární klíč, musí být přítomen"),
                    record.title,
                )
            })
            .fetch_all(conn.as_mut())
            .await
            .context("Nelze načíst seznam písní z databáze")
    }
}
