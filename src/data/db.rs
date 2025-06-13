//! Modul pro manipulaci s databází, načítání a ukládání dat.

use crate::data::Song;
use anyhow::{Context, Result};
use sqlx::{SqlitePool, query};

impl Song {
    /// Uloží danou píseň do lokální SQlite databáze, ke které se připojí pomocí `pool`.
    ///
    /// ### Ošetření chyb
    /// Chyba nastane pokud:
    /// - Píseň není validní (tag v pořadí, který se nevyskytuje ve slovech)
    /// - Píseň nebo její slova nesplňují integritní omezení databáze
    ///
    /// Pokud během ukládání písně do databáze nastane chyba, je proveden rollback celé písně.
    /// Tedy po chybě by databáze měla být ve stejném stavu jako před zavoláním této funkce.
    pub async fn save_to_db(&self, pool: &SqlitePool) -> Result<()> {
        self.check_order_validity()
            .context("Nelze uložit nevalidní píseň")?;

        let mut connection = pool
            .acquire()
            .await
            .context("Nelze získat připojení k databázi z poolu")?;

        let part_order = self.order.join(" ");

        let song_id = query!(
            "
            BEGIN TRANSACTION;
            INSERT INTO songs (title, author, part_order) VALUES ($1, $2, $3)
            ",
            self.title,
            self.author,
            part_order
        )
        .execute(&mut *connection)
        .await
        .context(format!("Nelze uložit píseň {} do databáze", self.title))?
        .last_insert_rowid();

        // TODO: Toto by šlo přepsat, abych místo sekvenčního ukládání spojil všechny query
        // do jedné future pomocí `join_all` a na tom awaitnout
        for (tag, lyrics) in self.parts.iter() {
            let query_result = query!(
                "INSERT INTO song_parts (song_id, tag, lyrics) VALUES ($1, $2, $3)",
                song_id,
                tag,
                lyrics
            )
            .execute(&mut *connection)
            .await;

            if let Err(e) = query_result {
                // Zrušení předcházejících INSERTů
                query!("ROLLBACK;")
                    .execute(&mut *connection)
                    .await
                    .context("Nelze provést ROLLBACK selhaného uložení písně")?;

                return Err(e).context(format!("Nelze uložit část {} písně {}", tag, self.title));
            }
        }

        query!("COMMIT;")
            .execute(&mut *connection)
            .await
            .context("Nelze provést COMMIT uložení písně")?;

        Ok(())
    }

    /// Načte píseň z SQLite databáze. Pokud se vyskytnou chyby při jejím čtení, vrátí Error.
    pub async fn load_from_db(pool: &SqlitePool) -> Result<Self> {
        todo!()
    }
}
