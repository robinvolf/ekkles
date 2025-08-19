//! Modul pro manipulaci s Playlisty.
//!
//! ### Datový model playlistu
//! Playlist je reprezentován dvojím způsobem:
//!   1. Pro prezentaci [`Playlist`], vlastní obsah všech položek (slova písní,
//!   obsah jednotlivých veršů)
//!   2. Pro modifikaci [`PlaylistMetadata`], ukládá pouze metadata položek (id z databáze),
//!   je určen pro editaci playlistu, a zpětné ukládání do databáze.
//!
//! ### Status Metadatového playlistu
//! Metadatový playlist má pole `status`, které označuje, zda-li jsou data uložena v DB.
//! Pracuje následovně:
//! ```text
//!            new()
//!             ->   Transient
//!                      |
//!          modify()    | save()
//!             <-       V
//!   Dirty             Clean
//!             ->
//!           save()
//! ```
//!
//! ### Ukládání času
//! Playlisty si ukládají čas vzniku, aby bylo možné je posléze podle něj řadit.
//! Tento čas je reprezentován jak v datovém modelu, tak v databázi jako UTC.
//!
//! Při interakci s uživatelem je pak dobré jej převést na lokální Timezone pomocí:
//!
//! ```rust,ignore
//! let utc = Utc::now();
//! let local: DateTime<Local> = DateTime::from(utc);
//! ```

use crate::{
    Song,
    bible::indexing::{Book, Passage, VerseIndex},
};
use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, NaiveDateTime, SubsecRound, Utc};
use sqlx::{Acquire, Sqlite, Transaction, pool::PoolConnection, query};

/// Hodnota sloupce 'kind' v tabulce 'playlist_parts' pro píseň
const DB_PLAYLIST_KIND_SONG: &str = "song";
/// Hodnota sloupce 'kind' v tabulce 'playlist_parts' pro pasáž z Bible
const DB_PLAYLIST_KIND_BIBLE_PASSAGE: &str = "bible";
/// Formátovací řetězec pro [`NaiveDateTime::parse_from_str`] a jí podobné funkce při
/// parsování řetězců z/do databáze.
const DB_DATETIME_FORMAT: &str = "%F %T";

/// Status playlistu ohledně databáze, viz [dokumentace modulu](`crate::playlist`)
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PlaylistMetadataStatus {
    /// Nebyl ještě uložen do databáze
    Transient,
    /// Je uložený v db pod daným ID
    Clean(i64),
    /// Je uložený v db pod daným ID, ale od svého uložení se liší (byl editován)
    Dirty(i64),
}

/// Playlist se skládá z vícero druhů položek, tento enum je rozlišuje.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum PlaylistItemMetadata {
    BiblePassage {
        translation_id: i64,
        from: VerseIndex,
        to: VerseIndex,
    },
    Song(i64),
}

/// Vrátí seznam všech playlistů v databázi. Vrátí dvojice (ID, název) seřazené podle
/// času vytvoření. Pokud se vyskytne chyba v databázi, vrátí Error
pub async fn get_available(mut conn: PoolConnection<Sqlite>) -> Result<Vec<(i64, String)>> {
    query!("SELECT id, name FROM playlists ORDER BY created ASC")
        .map(|record| (record.id, record.name))
        .fetch_all(&mut *conn)
        .await
        .context("Nelze načíst playlisty z databáze")
}

/// Pokud je název playlistu `name` k dispozici (zatím v databázi neexistuje
/// takto pojmenovaný playlist), vrátí `true`, jinak `false`. Pokud nastane
/// chyba s připojením k databázi, vrátí Error.
pub async fn is_name_available(mut conn: PoolConnection<Sqlite>, name: &str) -> Result<bool> {
    Ok(query!("SELECT id FROM playlists WHERE name == $1", name)
        .fetch_optional(&mut *conn)
        .await
        .context("Nelze se připojit k databázi")?
        .is_none())
}

impl PlaylistItemMetadata {
    /// Uloží danou položku playlistu `playlist_id` s pořadovým číslem `order` do databáze za pomocí dané transakce, pokud nastane chyba
    /// při ukládání, vrací Error.
    ///
    /// ### Transakce
    /// Volající je odpovědný za commit/rollback transakce, tato funkce pouze použije danou
    /// transakci k přístupu do databáze, ale commit neprovádí.
    async fn insert(
        &self,
        transaction: &mut Transaction<'_, Sqlite>,
        playlist_id: i64,
        order: u32,
    ) -> Result<()> {
        let kind = match self {
            PlaylistItemMetadata::BiblePassage { .. } => DB_PLAYLIST_KIND_BIBLE_PASSAGE,
            PlaylistItemMetadata::Song(_) => DB_PLAYLIST_KIND_SONG,
        };

        query!(
            "INSERT INTO playlist_parts (playlist_id, part_order, kind) VALUES ($1, $2, $3)",
            playlist_id,
            order,
            kind
        )
        .execute(&mut **transaction) // Docela prokleté, viz dokumentace Transaction v sqlx
        .await
        .context("Nelze vložit část playlistu")?;

        match self {
            PlaylistItemMetadata::BiblePassage {
                translation_id,
                from,
                to,
            } => {
                let (from_book, from_chapter, from_verse_number) = from.destructure_numeric();
                let (to_book, to_chapter, to_verse_number) = to.destructure_numeric();
                query!(
                        "INSERT INTO playlist_passages ( playlist_id, part_order, translation_id , start_book_id , start_chapter , start_number , end_book_id , end_chapter , end_number) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                        playlist_id,
                        order,
                        translation_id,
                        from_book,
                        from_chapter,
                        from_verse_number,
                        to_book,
                        to_chapter,
                        to_verse_number
                    )
                    .execute(&mut **transaction)
                    .await
                    .context("Nelze uložit pasáž do databáze")?;
            }
            PlaylistItemMetadata::Song(song_id) => {
                query!(
                        "INSERT INTO playlist_songs (playlist_id, part_order, song_id) VALUES ($1, $2, $3)",
                        playlist_id,
                        order,
                        song_id
                    )
                    .execute(&mut **transaction)
                    .await
                    .with_context(|| format!("Nelze uložit píseň s ID {} do databáze", song_id))?;
            }
        }

        Ok(())
    }

    /// Vloží do databáze všechny položky daného playlistu v daném pořadí.
    ///
    /// ### Transakce
    /// Používá dodanou transakci, je na volajícím, aby na jejím konci provedl commit.
    ///
    /// ### Pohled databáze
    /// Playlist by v db měl být prázdný, není to upsert, ale čistý insert, pokud nebude prázdný,
    /// shoří to na konfliktu při vkládání.
    async fn insert_many(
        items: &[Self],
        transaction: &mut Transaction<'_, Sqlite>,
        playlist_id: i64,
    ) -> Result<()> {
        for (order, item) in items.iter().enumerate() {
            let order: u32 = order.try_into().with_context(|| {
                format!(
                    "Playlist obsahuje více než {} položek (proč???), nelze uložit",
                    u32::MAX
                )
            })?;

            item.insert(transaction, playlist_id, order)
                .await
                .context("Nelze uložit položku playlistu")?;
        }

        Ok(())
    }

    /// Odstraní danou položku playlistu `playlist_id` s pořadovým číslem `order`
    /// z databáze za pomocí dané transakce, pokud nastane chyba při mazání, vrací Error.
    ///
    /// ### Existence v DB
    /// Pokud daná položka neexistuje v databázi vrací Error.
    ///
    /// ### Transakce
    /// Volající je odpovědný za commit/rollback transakce, tato funkce pouze použije danou
    /// transakci k přístupu do databáze, ale commit neprovádí.
    async fn delete(
        &self,
        transaction: &mut Transaction<'_, Sqlite>,
        playlist_id: i64,
        order: u32,
    ) -> Result<()> {
        let rows_affected = query!(
            "DELETE FROM playlist_parts WHERE playlist_id = $1 AND part_order = $2",
            playlist_id,
            order,
        )
        .execute(&mut **transaction) // Docela prokleté, viz dokumentace Transaction v sqlx
        .await
        .context("Nelze smazat část playlistu")?
        .rows_affected();

        if rows_affected == 0 {
            bail!("Část playlistu pro smazání neexistuje")
        }

        let rows_affected = match self {
            PlaylistItemMetadata::BiblePassage { .. } => query!(
                "DELETE FROM playlist_passages  WHERE playlist_id = $1 AND part_order = $2",
                playlist_id,
                order,
            )
            .execute(&mut **transaction)
            .await
            .context("Nelze smazat pasáž z playlistu")?
            .rows_affected(),
            PlaylistItemMetadata::Song(_) => query!(
                "DELETE FROM playlist_songs WHERE playlist_id = $1 AND part_order = $2",
                playlist_id,
                order,
            )
            .execute(&mut **transaction)
            .await
            .context("Nelze smazat píseň z playlistu")?
            .rows_affected(),
        };

        if rows_affected == 0 {
            bail!("Část playlistu pro smazání neexistuje")
        }

        Ok(())
    }

    /// Smaže všechny položky daného playlistu v databázi za pomocí dodané transakce.
    /// Pokud se vyskytne chyba, vrátí Error.
    ///
    /// ### Transakce
    /// Volající je odpovědný za commit/rollback transakce, tato funkce pouze použije danou
    /// transakci k přístupu do databáze, ale commit neprovádí.
    async fn delete_all(transaction: &mut Transaction<'_, Sqlite>, playlist_id: i64) -> Result<()> {
        query!(
            "DELETE FROM playlist_parts WHERE playlist_id = $1",
            playlist_id
        )
        .execute(&mut **transaction)
        .await
        .context("Nelze smazat části playlistu")?;

        query!(
            "DELETE FROM playlist_songs WHERE playlist_id = $1",
            playlist_id
        )
        .execute(&mut **transaction)
        .await
        .context("Nelze smazat písně playlistu")?;

        query!(
            "DELETE FROM playlist_passages WHERE playlist_id = $1",
            playlist_id
        )
        .execute(&mut **transaction)
        .await
        .context("Nelze smazat pasáže playlistu")?;

        Ok(())
    }

    /// Načte jednu položku daného playlistu s daným pořadovým číslem, pokud se načítání z databáze
    /// nepovede, vrací Error.
    async fn load_one(
        mut conn: PoolConnection<Sqlite>,
        playlist_id: i64,
        order: u32,
    ) -> Result<Self> {
        let kind = query!(
            "SELECT kind FROM playlist_parts WHERE playlist_id = $1 AND part_order = $2",
            playlist_id,
            order
        )
        .fetch_one(&mut *conn)
        .await
        .context("Nelze načíst druh položky playlistu")?
        .kind;

        match kind.as_str() {
            DB_PLAYLIST_KIND_SONG => {
                let song_id = query!(
                    "SELECT song_id FROM playlist_songs WHERE playlist_id = $1 AND part_order = $2",
                    playlist_id,
                    order
                )
                .fetch_one(&mut *conn)
                .await
                .with_context(|| {
                    format!(
                        "Nelze načíst část {} playlistu s id {} z databáze",
                        order, playlist_id
                    )
                })?
                .song_id;

                Ok(PlaylistItemMetadata::Song(song_id))
            }
            DB_PLAYLIST_KIND_BIBLE_PASSAGE => {
                let record = query!(
                        "SELECT translation_id, start_book_id, start_chapter, start_number, end_book_id, end_chapter, end_number FROM playlist_passages WHERE playlist_id = $1 AND part_order = $2",
                        playlist_id,
                        order
                    )
                    .fetch_one(&mut *conn)
                    .await
                    .with_context(|| {
                        format!(
                            "Nelze načíst část {} playlistu s id {} z databáze",
                            order, playlist_id
                        )
                    })?;

                let from = VerseIndex::try_new(
                    Book::try_from(record.start_book_id as u8)?,
                    record.start_chapter as u8,
                    record.start_number as u8,
                )
                .ok_or(anyhow!("Nevalidní index verše v databázi"))?;

                let to = VerseIndex::try_new(
                    Book::try_from(record.end_book_id as u8)?,
                    record.end_chapter as u8,
                    record.end_number as u8,
                )
                .ok_or(anyhow!("Nevalidní index verše v databázi"))?;

                Ok(PlaylistItemMetadata::BiblePassage {
                    translation_id: record.translation_id,
                    from,
                    to,
                })
            }
            _ => panic!(
                "Sloupec playlist_parts.kind by měl být integritně omezen na '{}' nebo '{}', došlo ke korupci dat v databázi?",
                DB_PLAYLIST_KIND_SONG, DB_PLAYLIST_KIND_BIBLE_PASSAGE
            ),
        }
    }

    /// Načte všechny položky playlistu a vrátí je jako vektor, pokud se načítání z databáze nepovede, vrací Error.
    async fn load_many(mut conn: PoolConnection<Sqlite>, playlist_id: i64) -> Result<Vec<Self>> {
        let parts = query!(
            "SELECT part_order, kind FROM playlist_parts WHERE playlist_id = $1 ORDER BY part_order ASC",
            playlist_id
        )
        .fetch_all(&mut *conn)
        .await
        .context("Nelze načíst část playlistu z databáze")?;

        let mut items = Vec::new();

        for record in parts {
            match record.kind.as_str() {
                DB_PLAYLIST_KIND_SONG => {
                    let song_id = query!(
                    "SELECT song_id FROM playlist_songs WHERE playlist_id = $1 AND part_order = $2",
                    playlist_id,
                    record.part_order
                    )
                    .fetch_one(&mut *conn)
                    .await
                    .with_context(|| {
                        format!(
                            "Nelze načíst část {} playlistu s id {} z databáze",
                            record.part_order, playlist_id
                        )
                    })?.song_id;

                    items.push(PlaylistItemMetadata::Song(song_id));
                }
                DB_PLAYLIST_KIND_BIBLE_PASSAGE => {
                    let record = query!(
                        "SELECT translation_id, start_book_id, start_chapter, start_number, end_book_id, end_chapter, end_number FROM playlist_passages WHERE playlist_id = $1 AND part_order = $2",
                        playlist_id,
                        record.part_order
                    )
                    .fetch_one(&mut *conn)
                    .await
                    .with_context(|| {
                        format!(
                            "Nelze načíst část {} playlistu s id {} z databáze",
                            record.part_order, playlist_id
                        )
                    })?;

                    let from = VerseIndex::try_new(
                        Book::try_from(record.start_book_id as u8)?,
                        record.start_chapter as u8,
                        record.start_number as u8,
                    )
                    .ok_or(anyhow!("Nevalidní index verše v databázi"))?;

                    let to = VerseIndex::try_new(
                        Book::try_from(record.end_book_id as u8)?,
                        record.end_chapter as u8,
                        record.end_number as u8,
                    )
                    .ok_or(anyhow!("Nevalidní index verše v databázi"))?;

                    let new_item = PlaylistItemMetadata::BiblePassage {
                        translation_id: record.translation_id,
                        from,
                        to,
                    };

                    items.push(new_item);
                }
                _ => panic!(
                    "Sloupec playlist_parts.kind by měl být integritně omezen na '{}' nebo '{}', došlo ke korupci dat v databázi?",
                    DB_PLAYLIST_KIND_SONG, DB_PLAYLIST_KIND_BIBLE_PASSAGE
                ),
            }
        }

        Ok(items)
    }
}

/// Struktura obsahující pouze metadata playlistu určená pro editaci
/// (nemusí načítat obsahy jednotlivých položek, postačí identifikátory).
///
/// Tato struktura reprezentuje playlist uložený v databázi a pomocí
/// [`PlaylistMetadata::get_status()`] lze zjistit, zda-li se od playlistu
/// v databázi liší (byl editován).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PlaylistMetadata {
    status: PlaylistMetadataStatus,
    name: String,
    /// Čas vytvoření playlistu zaokrouhlený k nejbližší sekundě
    created: DateTime<Utc>,
    items: Vec<PlaylistItemMetadata>,
}

impl PlaylistMetadata {
    /// Vytvoří nový playlist se jménem `name`.
    pub fn new(name: &str) -> Self {
        Self {
            status: PlaylistMetadataStatus::Transient,
            name: name.to_string(),
            created: Utc::now().round_subsecs(0),
            items: Vec::new(),
        }
    }

    /// Vytvoří nový playlist se jménem `name` a s položkami z `other`. Stav nového
    /// playlistu bude [`PlaylistMetadataStatus::Transient`] a čas jeho vytvoření
    /// bude čas zavolání této funkce.
    ///
    /// ### Druhý playlist
    /// Z druhého playlistu bude přesunut vektor s položkami.
    ///
    /// ### Proč ne move???
    /// Protože mutex!
    pub fn from_other(name: &str, other: &mut PlaylistMetadata) -> Self {
        let mut new = Self::new(name);
        std::mem::swap(&mut new.items, &mut other.items);
        new
    }

    /// Načte existující playlist s daným ID z databáze, status bude mít nastaven na
    /// [`PlaylistMetadataStatus::Clean`]. Pokud takový playlist neexistuje
    /// nebo se něco v pokazí při načítání, vrátí Error.
    pub async fn load(id: i64, mut conn: PoolConnection<Sqlite>) -> Result<Self> {
        let metadata = query!("SELECT name, created FROM playlists WHERE id = $1", id)
            .fetch_one(&mut *conn)
            .await
            .with_context(|| format!("Nelze načíst playlist s id {id} z databáze"))?;

        let name = metadata.name;
        let created = NaiveDateTime::parse_from_str(&metadata.created, DB_DATETIME_FORMAT)
            .with_context(|| format!("Nelze zparsovat datum z databáze {}", metadata.created))?
            .and_utc();

        let items = PlaylistItemMetadata::load_many(conn, id)
            .await
            .context("Nepodařilo se načíst položky playlistu")?;

        Ok(Self {
            status: PlaylistMetadataStatus::Clean(id),
            name,
            created,
            items,
        })
    }

    /// Získá status playlistu, viz: [`PlaylistMetadataStatus`]
    pub fn get_status(&self) -> PlaylistMetadataStatus {
        self.status
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Convenience funkce pro vkládání písní na konec playlistu. Má stejné chování jako [`PlaylistMetadata::add_song`].
    pub fn push_song(&mut self, song_id: i64) {
        self.add_song(song_id, self.items.len());
    }

    /// Přidá píseň s ID `song_id` do playlistu na pozici `position`. Pokud byl status `clean`, shodí jej na `dirty`.
    pub fn add_song(&mut self, song_id: i64, position: usize) {
        self.items
            .insert(position, PlaylistItemMetadata::Song(song_id));

        if let PlaylistMetadataStatus::Clean(id) = self.status {
            self.status = PlaylistMetadataStatus::Dirty(id);
        }
    }

    /// Convenience funkce pro vkládání pasáží na konec playlistu. Má stejné chování jako [`PlaylistMetadata::add_bible_passage`].
    pub fn push_bible_passage(&mut self, translation_id: i64, from: VerseIndex, to: VerseIndex) {
        self.add_bible_passage(translation_id, from, to, self.items.len());
    }

    /// Přidá pasáž do playlistu na pozici `position`. Pasáž bude z překladu s ID `translation_id` a bude od `from` do `to`. Pokud byl status `clean`, shodí jej na `dirty`.
    pub fn add_bible_passage(
        &mut self,
        translation_id: i64,
        from: VerseIndex,
        to: VerseIndex,
        position: usize,
    ) {
        self.items.insert(
            position,
            PlaylistItemMetadata::BiblePassage {
                translation_id,
                from,
                to,
            },
        );

        if let PlaylistMetadataStatus::Clean(id) = self.status {
            self.status = PlaylistMetadataStatus::Dirty(id);
        }
    }

    /// Odstraní položku na indexu `position` z playlistu, pokud na tomto indexu neexistje
    /// položka, vrací Error. Pokud byl status `clean`, shodí jej na `dirty`.
    pub fn delete_item(&mut self, position: usize) -> Result<()> {
        if self.items.len() <= position {
            bail!("Položka na indexu {position} neexistuje");
        } else {
            self.items.remove(position);

            if let PlaylistMetadataStatus::Clean(id) = self.status {
                self.status = PlaylistMetadataStatus::Dirty(id);
            }

            Ok(())
        }
    }

    /// Prohodí položky na pozicích `a` a `b` v playlistu, pokud je jeden index mimo vektor, vrací error. Pokud byl status `clean`, shodí jej na `dirty`.
    pub fn swap_items(&mut self, a: usize, b: usize) -> Result<()> {
        if self.items.get(a).is_none() {
            bail!(
                "Index {a} je mimo rozsah položek (0 až {})",
                self.items.len() - 1
            );
        } else if self.items.get(b).is_none() {
            bail!(
                "Index {b} je mimo rozsah položek (0 až {})",
                self.items.len() - 1
            );
        } else {
            self.items.swap(a, b);

            if let PlaylistMetadataStatus::Clean(id) = self.status {
                self.status = PlaylistMetadataStatus::Dirty(id);
            }

            Ok(())
        }
    }

    /// Uloží daný playlist do databáze a nastaví jeho status na [`PlaylistMetadataStatus::Clean`].
    /// Pokud je již status playlistu [`PlaylistMetadataStatus::Clean`], je tato metoda no-op.
    pub async fn save(&mut self, conn: PoolConnection<Sqlite>) -> Result<()> {
        match self.status {
            PlaylistMetadataStatus::Transient => {
                let new_id = self.save_transient(conn).await?;
                self.status = PlaylistMetadataStatus::Clean(new_id);
                Ok(())
            }
            PlaylistMetadataStatus::Clean(_) => Ok(()),
            PlaylistMetadataStatus::Dirty(_) => self.save_dirty(conn).await,
        }
    }

    /// Uloží "špinavý" playlist do databáze a označí jej jako čistý, pokud se nepovede, vrací Error.
    ///
    /// ### Bezpečnost
    /// Tato metoda musí být volána *pouze* na playlistech, které mají status [`PlaylistMetadataStatus::Dirty`], jinak metoda zpanikaří.
    /// Toto je low-level metoda, pro uložení playlistu bys měl použít raději [`PlaylistMetadata::save()`].
    ///
    /// ### Integrita databáze
    /// Tato metoda používá [transakce](sqlx::Transaction), pokud jakákoliv část ukládání selže,
    /// bude proveden rollback a databáze zůstane ve stavu, v jakém byla před voláním metody.
    async fn save_dirty(&mut self, mut conn: PoolConnection<Sqlite>) -> Result<()> {
        let id = if let PlaylistMetadataStatus::Dirty(id) = self.status {
            id
        } else {
            panic!(
                "Metoda `save_dirty()` byla zavolána na ne-dirty playlistu, toto by se nikdy nemělo stát"
            )
        };

        let mut transaction = conn
            .begin()
            .await
            .context("Nelze získat transakci na poolu databáze")?;

        // Update jména
        query!(
            "UPDATE playlists SET name = $1 WHERE id = $2",
            self.name,
            id
        )
        .execute(&mut *transaction)
        .await
        .context("Nelze updatovat jméno playlistu")?;

        // Odstranění všech starých položek
        PlaylistItemMetadata::delete_all(&mut transaction, id)
            .await
            .context("Nelze smazat staré položky playlistu")?;

        // Vložení nových položek
        PlaylistItemMetadata::insert_many(&self.items, &mut transaction, id)
            .await
            .context("Nelze vložit nové položky playlistu")?;

        transaction
            .commit()
            .await
            .with_context(|| format!("Commit transakce uložení playlistu {} selhal", self.name))
    }

    /// Uloží čerstvý playlist do databáze, playlist byl pouze v paměti. V případě úspěchu vrátí  ID pod kterým byl playlist uložen, v opačném případě vrací Error.
    ///
    /// ### Bezpečnost
    /// Tato metoda musí být volána *pouze* na playlistech, které mají status [`PlaylistMetadataStatus::Clean`], jinak metoda zpanikaří.
    /// Toto je low-level metoda, pro uložení playlistu bys měl použít raději [`PlaylistMetadata::save()`].
    ///
    /// ### Integrita databáze
    /// Tato metoda používá [transakce](sqlx::Transaction), pokud jakákoliv část ukládání selže,
    /// bude proveden rollback a databáze zůstane ve stavu, v jakém byla před voláním metody.
    async fn save_transient(&self, mut conn: PoolConnection<Sqlite>) -> Result<i64> {
        assert_eq!(
            self.status,
            PlaylistMetadataStatus::Transient,
            "Metoda `save_transient()` byla zavolána na ne-transient playlistu, toto by se nikdy nemělo stát"
        );

        let mut transaction = conn
            .begin()
            .await
            .context("Nelze získat transakci na poolu databáze")?;

        let formatted_datetime = self.created.format(DB_DATETIME_FORMAT).to_string();

        let playlist_id = query!(
            "INSERT INTO playlists (name, created) VALUES ($1, datetime($2))",
            self.name,
            formatted_datetime
        )
        .execute(&mut *transaction)
        .await
        .with_context(|| format!("Nelze uložit playlist '{}' do databáze", self.name))?
        .last_insert_rowid();

        for (order, item) in self.items.iter().enumerate() {
            let order: u32 = order.try_into().with_context(|| {
                format!(
                    "Playlist obsahuje více než {} položek (proč???), nelze uložit",
                    u32::MAX
                )
            })?;

            let item_kind = match item {
                PlaylistItemMetadata::BiblePassage { .. } => DB_PLAYLIST_KIND_BIBLE_PASSAGE,
                PlaylistItemMetadata::Song(_) => DB_PLAYLIST_KIND_SONG,
            };

            query!(
                "INSERT INTO playlist_parts (playlist_id, part_order, kind) VALUES ($1, $2, $3)",
                playlist_id,
                order,
                item_kind
            )
            .execute(&mut *transaction)
            .await
            .with_context(|| format!("Nelze uložit část playlistu '{}' do databáze", self.name))?;

            match item {
                PlaylistItemMetadata::BiblePassage {
                    translation_id,
                    from,
                    to,
                } => {
                    let (from_book, from_chapter, from_verse_number) = from.destructure_numeric();
                    let (to_book, to_chapter, to_verse_number) = to.destructure_numeric();
                    query!(
                        "INSERT INTO playlist_passages ( playlist_id, part_order, translation_id , start_book_id , start_chapter , start_number , end_book_id , end_chapter , end_number) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                        playlist_id,
                        order,
                        translation_id,
                        from_book,
                        from_chapter,
                        from_verse_number,
                        to_book,
                        to_chapter,
                        to_verse_number
                    )
                    .execute(&mut *transaction)
                    .await
                    .with_context(|| format!("Nelze uložit pasáž playlistu '{}' do databáze", self.name))?; // TODO: Tu pasáž lze i pojmenovat, až budeme mít Display pro Passage/VerseIndex
                }
                PlaylistItemMetadata::Song(song_id) => {
                    query!(
                        "INSERT INTO playlist_songs (playlist_id, part_order, song_id) VALUES ($1, $2, $3)",
                        playlist_id,
                        order,
                        song_id
                    )
                    .execute(&mut *transaction)
                    .await
                    .with_context(|| format!("Nelze uložit píseň s ID {} playlistu '{}' do databáze", song_id, self.name))?;
                }
            }
        }

        transaction
            .commit()
            .await
            .with_context(|| format!("Commit transakce uložení playlistu {} selhal", self.name))?;

        Ok(playlist_id)
    }
}

/// Co všechno může být rozdíl mezi dvěma [`PlaylistMetadata`].
#[derive(Debug, PartialEq, Eq)]
enum PlaylistMetadataDiff {
    /// Jiný název
    Name(String),
    /// Přidaná položka
    Added(PlaylistItemMetadata),
    /// Odstraněná položka
    Removed(PlaylistItemMetadata),
}

#[derive(Debug)]
/// Playlist se skládá z vícero druhů položek, tento enum je rozlišuje.
enum PlaylistItem {
    BiblePassage(Passage),
    Song(Song),
}

/// Struktura reprezentující playlist, která vlastní obsah svých položek. Je tedy "nezávislá",
/// je možné použít čistě tuto strukturu a bez dalších přístupů do databáze z ní vytvořit
/// promítatelné slajdy.
#[derive(Debug)]
pub struct Playlist {
    id: i64,
    name: String,
    created: DateTime<Utc>,
    items: Vec<PlaylistItem>,
}

impl Playlist {
    /// Načte playlist s daným ID z databáze.
    pub async fn load(id: i64, mut conn: PoolConnection<Sqlite>) -> Result<Self> {
        let playlist_record = query!("SELECT id, name, created FROM playlists WHERE id = $1", id)
            .fetch_one(&mut *conn)
            .await
            .with_context(|| format!("Playlist s id {id} nebyl nalezen"))?;

        let name = playlist_record.name;
        let created = NaiveDateTime::parse_from_str(&playlist_record.created, DB_DATETIME_FORMAT)
            .with_context(|| {
                format!(
                    "Nelze zparsovat datum z databáze {}",
                    playlist_record.created
                )
            })?
            .and_utc();

        let parts = query!(
            "SELECT part_order, kind FROM playlist_parts WHERE playlist_id = $1 ORDER BY part_order ASC",
            id
        ).fetch_all(&mut *conn).await
            .context("Nelze načíst další část playlistu z databáze")?
        ;

        // Pořadí vkládání nemusíme řešit, z databáze to přijde již seřazené
        let mut items = Vec::new();

        for part_record in parts {
            match part_record.kind.as_str() {
                DB_PLAYLIST_KIND_BIBLE_PASSAGE => {
                    let song_id = query!(
                        "SELECT song_id FROM playlist_songs WHERE playlist_id = $1 AND part_order = $2",
                        id,
                        part_record.part_order
                    ).fetch_one(&mut *conn).await.with_context(|| format!("Nelze načíst píseň do playlistu s id {} a pořadovým číslem {}", id, part_record.part_order))?.song_id;

                    let song = Song::load_from_db(song_id, &mut conn)
                        .await
                        .context("Nelze načíst píseň do playlistu")?;

                    items.push(PlaylistItem::Song(song));
                }
                DB_PLAYLIST_KIND_SONG => {
                    let passage_record = query!(
                        "SELECT translation_id , start_book_id , start_chapter , start_number , end_book_id , end_chapter , end_number FROM playlist_passages WHERE playlist_id = $1 AND part_order = $2",
                        id,
                        part_record.part_order
                    ).fetch_one(&mut *conn).await.with_context(|| format!("Nelze načíst píseň do playlistu s id {} a pořadovým číslem {}", id, part_record.part_order))?;

                    let start = VerseIndex::try_new(
                        (passage_record.start_book_id as u8).try_into().unwrap(),
                        passage_record.start_chapter as u8,
                        passage_record.start_number as u8,
                    )
                    .with_context(|| {
                        format!(
                            "Nelze najít verš v knize {}, kapitole {} s číslem {}",
                            passage_record.start_book_id,
                            passage_record.start_chapter,
                            passage_record.start_number
                        )
                    })?;

                    let end = VerseIndex::try_new(
                        (passage_record.end_book_id as u8).try_into().unwrap(),
                        passage_record.end_chapter as u8,
                        passage_record.end_number as u8,
                    )
                    .with_context(|| {
                        format!(
                            "Nelze najít verš v knize {}, kapitole {} s číslem {}",
                            passage_record.end_book_id,
                            passage_record.end_chapter,
                            passage_record.end_number
                        )
                    })?;

                    let passage =
                        Passage::load(start, end, passage_record.translation_id, &mut conn)
                            .await
                            .with_context(|| {
                                format!(
                                    "Nelze načíst pasáž od {:?} do {:?} v překladu {}",
                                    start, end, passage_record.translation_id
                                )
                            })?;

                    items.push(PlaylistItem::BiblePassage(passage));
                }
                _ => bail!("Neznámý druh části playlistu: {}", part_record.kind),
            }
        }

        Ok(Self {
            id,
            name,
            created,
            items,
        })
    }
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;
    use sqlx::{SqlitePool, query_file};

    use super::*;

    /// Funkce na vytvoření in-memory databáze pro testování. Vytvoří holou databázi
    /// a nasype do ní dvě písně a prvních 10 veršů genesis pro testování. Též vytvoří
    /// prázdný playlist s ID 0. Pro detaily viz soubory ve `query_file!()`
    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        query_file!("db/init_db.sql").execute(&pool).await.unwrap();

        query_file!("db/fill_test_db.sql")
            .execute(&pool)
            .await
            .unwrap();

        // Vložíme testovací playlist na kterém se budou operace na itemech zkoušet
        query!("INSERT INTO playlists (id, 'name') VALUES (0, 'test')")
            .execute(&pool)
            .await
            .unwrap();

        pool
    }

    #[tokio::test]
    async fn metadata_item_insert_song_test() {
        let pool = setup_test_db().await;

        let song = PlaylistItemMetadata::Song(0);

        let mut tx1 = pool.begin().await.unwrap();

        let playlist_id = 0;
        let song_order = 0;
        let res = song.insert(&mut tx1, playlist_id, song_order).await;
        assert!(res.is_ok());

        tx1.commit().await.unwrap();

        let (order, kind) = query!("SELECT * FROM playlist_parts WHERE playlist_id = 0")
            .map(|record| (record.part_order, record.kind))
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(order as u32, song_order);
        assert_eq!(kind, DB_PLAYLIST_KIND_SONG);

        let song_id_from_db = query!(
            "SELECT * FROM playlist_songs WHERE playlist_id = 0 AND part_order = $1",
            song_order
        )
        .map(|record| record.song_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(song_id_from_db, 0);
    }

    #[tokio::test]
    async fn metadata_item_insert_passage_test() {
        let pool = setup_test_db().await;

        let bible_passage = PlaylistItemMetadata::BiblePassage {
            translation_id: 0,
            from: VerseIndex::try_new(Book::Genesis, 1, 1).unwrap(),
            to: VerseIndex::try_new(Book::Genesis, 1, 10).unwrap(),
        };

        let mut tx1 = pool.begin().await.unwrap();

        let playlist_id = 0;
        let passage_order = 0;
        let res = bible_passage
            .insert(&mut tx1, playlist_id, passage_order)
            .await;
        assert!(res.is_ok());

        tx1.commit().await.unwrap();

        let (order, kind) = query!("SELECT * FROM playlist_parts WHERE playlist_id = 0")
            .map(|record| (record.part_order, record.kind))
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(order as u32, passage_order);
        assert_eq!(kind, DB_PLAYLIST_KIND_BIBLE_PASSAGE);

        let passage_from_db = query!(
            "SELECT * FROM playlist_passages WHERE playlist_id = 0 AND part_order = $1",
            passage_order
        )
        .map(|record| PlaylistItemMetadata::BiblePassage {
            translation_id: record.translation_id,
            from: VerseIndex::try_new(
                Book::try_from(record.start_book_id as u8).unwrap(),
                record.start_chapter as u8,
                record.start_number as u8,
            )
            .unwrap(),
            to: VerseIndex::try_new(
                Book::try_from(record.end_book_id as u8).unwrap(),
                record.end_chapter as u8,
                record.end_number as u8,
            )
            .unwrap(),
        })
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(passage_from_db, bible_passage);
    }

    #[tokio::test]
    async fn metadata_item_insert_same_order_test() {
        let pool = setup_test_db().await;

        let song = PlaylistItemMetadata::Song(0);
        let bible_passage = PlaylistItemMetadata::BiblePassage {
            translation_id: 0,
            from: VerseIndex::try_new(Book::Genesis, 1, 1).unwrap(),
            to: VerseIndex::try_new(Book::Genesis, 1, 10).unwrap(),
        };

        let mut tx1 = pool.begin().await.unwrap();

        let playlist_id = 0;
        let same_order = 0;

        let res = bible_passage
            .insert(&mut tx1, playlist_id, same_order)
            .await;
        assert!(res.is_ok());

        let res = song.insert(&mut tx1, playlist_id, same_order).await;

        assert!(res.is_err());
    }

    #[tokio::test]
    async fn metadata_item_load_one_test() {
        let pool = setup_test_db().await;

        let song = PlaylistItemMetadata::Song(0);
        let bible_passage = PlaylistItemMetadata::BiblePassage {
            translation_id: 0,
            from: VerseIndex::try_new(Book::Genesis, 1, 1).unwrap(),
            to: VerseIndex::try_new(Book::Genesis, 1, 10).unwrap(),
        };

        let mut tx1 = pool.begin().await.unwrap();

        let playlist_id = 0;
        let passage_order = 0;
        let song_order = 1;

        let res = bible_passage
            .insert(&mut tx1, playlist_id, passage_order)
            .await;
        assert!(res.is_ok());
        let res = song.insert(&mut tx1, playlist_id, song_order).await;
        assert!(res.is_ok());

        tx1.commit().await.unwrap();

        let passage_from_db = PlaylistItemMetadata::load_one(
            pool.acquire().await.unwrap(),
            playlist_id,
            passage_order,
        )
        .await
        .ok();
        let song_from_db =
            PlaylistItemMetadata::load_one(pool.acquire().await.unwrap(), playlist_id, song_order)
                .await
                .ok();

        assert_eq!(passage_from_db, Some(bible_passage));
        assert_eq!(song_from_db, Some(song));
    }

    #[tokio::test]
    async fn metadata_item_load_many_test() {
        let pool = setup_test_db().await;

        let song = PlaylistItemMetadata::Song(0);
        let bible_passage = PlaylistItemMetadata::BiblePassage {
            translation_id: 0,
            from: VerseIndex::try_new(Book::Genesis, 1, 1).unwrap(),
            to: VerseIndex::try_new(Book::Genesis, 1, 10).unwrap(),
        };

        let mut tx1 = pool.begin().await.unwrap();

        let playlist_id = 0;
        let passage_order = 0;
        let song_order = 1;

        let res = bible_passage
            .insert(&mut tx1, playlist_id, passage_order)
            .await;
        assert!(res.is_ok());
        let res = song.insert(&mut tx1, playlist_id, song_order).await;
        assert!(res.is_ok());

        tx1.commit().await.unwrap();

        let items =
            PlaylistItemMetadata::load_many(pool.acquire().await.unwrap(), playlist_id).await;

        assert!(items.is_ok());

        assert_eq!(items.unwrap(), vec![bible_passage, song]);
    }

    #[tokio::test]
    async fn metadata_item_delete_test() {
        let pool = setup_test_db().await;

        let song = PlaylistItemMetadata::Song(0);
        let bible_passage = PlaylistItemMetadata::BiblePassage {
            translation_id: 0,
            from: VerseIndex::try_new(Book::Genesis, 1, 1).unwrap(),
            to: VerseIndex::try_new(Book::Genesis, 1, 10).unwrap(),
        };

        let mut tx1 = pool.begin().await.unwrap();

        let playlist_id = 0;
        let passage_order = 0;
        let song_order = 1;

        let res = bible_passage
            .insert(&mut tx1, playlist_id, passage_order)
            .await;
        assert!(res.is_ok());
        let res = song.insert(&mut tx1, playlist_id, song_order).await;
        assert!(res.is_ok());

        tx1.commit().await.unwrap();

        let mut tx2 = pool.begin().await.unwrap();

        let res = bible_passage
            .delete(&mut tx2, playlist_id, passage_order)
            .await;

        assert!(res.is_ok());

        tx2.commit().await.unwrap();

        let res = PlaylistItemMetadata::load_one(
            pool.acquire().await.unwrap(),
            playlist_id,
            passage_order,
        )
        .await;

        assert!(res.is_err());
    }

    #[tokio::test]
    async fn metadata_item_delete_nonexistent_test() {
        let pool = setup_test_db().await;

        let bible_passage = PlaylistItemMetadata::BiblePassage {
            translation_id: 0,
            from: VerseIndex::try_new(Book::Genesis, 1, 1).unwrap(),
            to: VerseIndex::try_new(Book::Genesis, 1, 10).unwrap(),
        };

        let playlist_id = 0;
        let passage_order = 0;

        let mut tx1 = pool.begin().await.unwrap();

        let res = bible_passage
            .delete(&mut tx1, playlist_id, passage_order)
            .await;

        dbg!(&res);
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn metadata_item_delete_all_test() {
        let pool = setup_test_db().await;

        let song = PlaylistItemMetadata::Song(0);
        let bible_passage = PlaylistItemMetadata::BiblePassage {
            translation_id: 0,
            from: VerseIndex::try_new(Book::Genesis, 1, 1).unwrap(),
            to: VerseIndex::try_new(Book::Genesis, 1, 10).unwrap(),
        };

        let mut tx1 = pool.begin().await.unwrap();

        let playlist_id = 0;
        let passage_order = 0;
        let song_order = 1;

        let res = bible_passage
            .insert(&mut tx1, playlist_id, passage_order)
            .await;
        assert!(res.is_ok());
        let res = song.insert(&mut tx1, playlist_id, song_order).await;
        assert!(res.is_ok());

        tx1.commit().await.unwrap();

        let mut tx2 = pool.begin().await.unwrap();

        let res = PlaylistItemMetadata::delete_all(&mut tx2, playlist_id).await;

        assert!(res.is_ok());

        tx2.commit().await.unwrap();

        let res = PlaylistItemMetadata::load_many(pool.acquire().await.unwrap(), playlist_id).await;

        assert!(res.is_ok_and(|vec| vec.is_empty()))
    }
}
