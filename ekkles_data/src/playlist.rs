use crate::{
    Song,
    bible::indexing::{Passage, VerseIndex},
};
use anyhow::{Context, Result, bail};
use chrono::NaiveDateTime;
use futures::TryStreamExt;
use sqlx::{SqlitePool, query};

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
struct Playlist {
    id: i64,
    name: String,
    created: NaiveDateTime,
    items: Vec<PlaylistItem>,
}

/// Hodnota sloupce 'kind' v tabulce 'playlist_parts' pro píseň
const DB_PLAYLIST_KIND_SONG: &str = "song";
/// Hodnota sloupce 'kind' v tabulce 'playlist_parts' pro pasáž z Bible
const DB_PLAYLIST_KIND_BIBLE_PASSAGE: &str = "bible";

impl Playlist {
    /// Načte playlist s daným ID z databáze.
    async fn load(id: i64, pool: &SqlitePool) -> Result<Self> {
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
            })?;

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
