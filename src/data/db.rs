//! Modul pro manipulaci s databází, načítání a ukládání dat.

use std::collections::HashMap;

use crate::data::Song;
use anyhow::{Context, Result};
use iced::futures::TryStreamExt;
use sqlx::{SqlitePool, query};

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

    /// Načte píseň s `id` z SQLite databáze pomocí `pool`.
    ///
    /// ### Ošetření chyb
    /// Vrátí Error, když:
    /// - Se vyskytnou chyby při čtení z databáze
    /// - Načtená píseň nesplňuje invariant (viz dokumentace [Song])
    pub async fn load_from_db(id: i64, pool: &SqlitePool) -> Result<Self> {
        let record = query!(
            "SELECT title, author, part_order FROM songs WHERE id = $1",
            id
        )
        .fetch_one(pool)
        .await
        .with_context(|| format!("Píseň s id {id} nebyla nalezena"))?;

        let title = record.title;
        let author = record.author;
        let order: Vec<String> = record
            .part_order
            .split(TAG_SPLIT_STRING)
            .map(|str| str.to_string())
            .collect();

        let mut lyrics =
            query!("SELECT tag, lyrics FROM song_parts WHERE song_id = $1", id).fetch(pool);

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
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::data::bible::tests::setup_db;

    #[tokio::test]
    async fn save_load_happy_path() {
        let pool = setup_db().await;

        let song = Song {
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

        let id = match song.save_to_db(&pool).await {
            Ok(id) => id,
            Err(e) => {
                println!("{:?}", e);
                panic!()
            }
        };

        match Song::load_from_db(id, &pool).await {
            Ok(loaded_song) => assert_eq!(loaded_song, song),
            Err(e) => {
                println!("{:?}", e);
                panic!()
            }
        }
    }

    #[tokio::test]
    async fn save_corrupted_song() {
        let pool = setup_db().await;

        let song = Song {
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
                String::from("Neexistující_tag"),
            ],
        };

        assert!(song.save_to_db(&pool).await.is_err());
    }
}
