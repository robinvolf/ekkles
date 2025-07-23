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
//! ```
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
//! ```rust
//! let utc = Utc::now();
//! let local: DateTime<Local> = DateTime::from(utc);
//! ```

use crate::{
    Song,
    bible::indexing::{Passage, VerseIndex},
};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveDateTime, Utc};
use futures::TryStreamExt;
use sqlx::{SqlitePool, query};

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
#[derive(Debug)]
enum PlaylistItemMetadata {
    BiblePassage {
        translation_id: i64,
        from: VerseIndex,
        to: VerseIndex,
    },
    Song(i64),
}

/// Struktura obsahující pouze metadata playlistu určená pro editaci
/// (nemusí načítat obsahy jednotlivých položek, postačí identifikátory).
///
/// Tato struktura reprezentuje playlist uložený v databázi a pomocí
/// [`PlaylistMetadata::get_status()`] lze zjistit, zda-li se od playlistu
/// v databázi liší (byl editován).
#[derive(Debug)]
pub struct PlaylistMetadata {
    status: PlaylistMetadataStatus,
    name: String,
    created: DateTime<Utc>,
    items: Vec<PlaylistItemMetadata>,
}

impl PlaylistMetadata {
    /// Vytvoří nový playlist se jménem `name`.
    pub fn new(name: &str) -> Self {
        Self {
            status: PlaylistMetadataStatus::Transient,
            name: name.to_string(),
            created: Utc::now(),
            items: Vec::new(),
        }
    }

    /// Získá status playlistu, viz: [`PlaylistMetadataStatus`]
    pub fn get_status(&self) -> PlaylistMetadataStatus {
        self.status
    }

    /// Přidá píseň s ID `song_id` do playlistu. Pokud byl status `clean`, shodí jej na `dirty`.
    pub fn add_song(&mut self, song_id: i64) {
        self.items.push(PlaylistItemMetadata::Song(song_id));

        if let PlaylistMetadataStatus::Clean(id) = self.status {
            self.status = PlaylistMetadataStatus::Dirty(id);
        }
    }

    /// Uloží daný playlist do databáze a nastaví jeho status na [`PlaylistMetadataStatus::Clean`].
    /// Pokud je již status playlistu [`PlaylistMetadataStatus::Clean`], je tato metoda no-op.
    pub async fn save(&mut self, pool: &SqlitePool) -> Result<()> {
        match self.status {
            PlaylistMetadataStatus::Transient => {
                let new_id = self.save_transient(pool).await?;
                self.status = PlaylistMetadataStatus::Clean(new_id);
                Ok(())
            }
            PlaylistMetadataStatus::Clean(_) => Ok(()),
            PlaylistMetadataStatus::Dirty(_) => todo!(),
        }
    }

    /// Uloží čerstvý playlist do databáze, playlist byl pouze v paměti. V případě úspěchu vrátí  ID pod kterým byl playlist uložen, v opačném případě vrací Error.
    ///
    /// ### Integrita
    /// Tato metoda musí být volána *pouze* na playlistech, které mají status [`PlaylistMetadataStatus::Clean`], jinak metoda zpanikaří.
    /// Toto je low-level metoda, pro uložení playlistu bys měl použít raději [`PlaylistMetadata::save()`].
    async fn save_transient(&self, pool: &SqlitePool) -> Result<i64> {
        assert_eq!(
            self.status,
            PlaylistMetadataStatus::Transient,
            "Metoda `save_transient()` byla zavolána na ne-transient playlistu, toto by se nikdy nemělo stát"
        );

        let mut transation = pool
            .begin()
            .await
            .context("Nelze získat transakci na poolu databáze")?;

        let formatted_datetime = self.created.format("%F %T").to_string();

        let playlist_id = query!(
            "INSERT INTO playlists (name, created) VALUES ($1, datetime($2))",
            self.name,
            formatted_datetime
        )
        .execute(&mut *transation)
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
            .execute(&mut *transation)
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
                        "INSERT INTO playlist_passages ( playlist_id, part_order, start_translation_id , start_book_id , start_chapter , start_number , end_translation_id , end_book_id , end_chapter , end_number) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                        playlist_id,
                        order,
                        translation_id,
                        from_book,
                        from_chapter,
                        from_verse_number,
                        translation_id,
                        to_book,
                        to_chapter,
                        to_verse_number
                    )
                    .execute(&mut *transation)
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
                    .execute(&mut *transation)
                    .await
                    .with_context(|| format!("Nelze uložit píseň s ID {} playlistu '{}' do databáze", song_id, self.name))?;
                }
            }
        }

        transation
            .commit()
            .await
            .with_context(|| format!("Commit transakce uložení playlistu {} selhal", self.name))?;

        Ok(playlist_id)
    }
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

/// Hodnota sloupce 'kind' v tabulce 'playlist_parts' pro píseň
const DB_PLAYLIST_KIND_SONG: &str = "song";
/// Hodnota sloupce 'kind' v tabulce 'playlist_parts' pro pasáž z Bible
const DB_PLAYLIST_KIND_BIBLE_PASSAGE: &str = "bible";

impl Playlist {
    /// Načte playlist s daným ID z databáze.
    pub async fn load(id: i64, pool: &SqlitePool) -> Result<Self> {
        let playlist_record = query!("SELECT id, name, created FROM playlists WHERE id = $1", id)
            .fetch_one(pool)
            .await
            .with_context(|| format!("Playlist s id {id} nebyl nalezen"))?;

        let name = playlist_record.name;
        let created = NaiveDateTime::parse_from_str(&playlist_record.created, "%Y-%m-%d %H:%M:%S")
            .with_context(|| {
                format!(
                    "Nelze zparsovat datum z databáze {}",
                    playlist_record.created
                )
            })?
            .and_utc();

        let mut parts = query!(
            "SELECT part_order, kind FROM playlist_parts WHERE playlist_id = $1 ORDER BY part_order ASC",
            id
        ).fetch(pool);

        // Pořadí vkládání nemusíme řešit, z databáze to přijde již seřazené
        let mut items = Vec::new();

        while let Some(part_record) = parts
            .try_next()
            .await
            .context("Nelze načíst další část playlistu z databáze")?
        {
            match part_record.kind.as_str() {
                DB_PLAYLIST_KIND_BIBLE_PASSAGE => {
                    let song_id = query!(
                        "SELECT song_id FROM playlist_songs WHERE playlist_id = $1 AND part_order = $2",
                        id,
                        part_record.part_order
                    ).fetch_one(pool).await.with_context(|| format!("Nelze načíst píseň do playlistu s id {} a pořadovým číslem {}", id, part_record.part_order))?.song_id;

                    let song = Song::load_from_db(song_id, pool)
                        .await
                        .context("Nelze načíst píseň do playlistu")?;

                    items.push(PlaylistItem::Song(song));
                }
                DB_PLAYLIST_KIND_SONG => {
                    let passage_record = query!(
                        "SELECT start_translation_id , start_book_id , start_chapter , start_number , end_translation_id , end_book_id , end_chapter , end_number FROM playlist_passages WHERE playlist_id = $1 AND part_order = $2",
                        id,
                        part_record.part_order
                    ).fetch_one(pool).await.with_context(|| format!("Nelze načíst píseň do playlistu s id {} a pořadovým číslem {}", id, part_record.part_order))?;

                    if passage_record.start_translation_id != passage_record.end_translation_id {
                        bail!("Nelze načíst pasáž se začátkem a koncem v jiných překladech");
                    }

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
                        Passage::load(start, end, passage_record.start_translation_id, pool)
                            .await
                            .with_context(|| {
                                format!(
                                    "Nelze načíst pasáž od {:?} do {:?} v překladu {}",
                                    start, end, passage_record.start_translation_id
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
