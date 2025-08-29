use anyhow::{Result, anyhow};
use ekkles_data::playlist::PlaylistItem;
use ekkles_data::{bible::indexing::VerseIndex, playlist::Playlist};
use iced::Element;

/// Počet veršů na jeden slajd, proteď konstanta
const VERSES_PER_SLIDE: usize = 2;

enum Slide {
    Passage(PassageSlide),
    Song(SongSlide),
}

impl Slide {
    fn present(&self) -> Element<crate::Message> {
        match self {
            Slide::Passage(passage_slide) => passage_slide.present(),
            Slide::Song(song_slide) => song_slide.present(),
        }
    }
}

/// Jeden slajd při promítání pasáže
struct PassageSlide {
    /// Název překladu, ze které je pasáž přebraná
    translation_name: String,
    /// Indexy celkové pasáže od-do
    passage_indexes: (VerseIndex, VerseIndex),
    /// Jednotlivé verše daného slajdu
    verses: Vec<(u8, String)>,
}

impl PassageSlide {
    fn new(
        translation_name: String,
        from: VerseIndex,
        to: VerseIndex,
        verses: Vec<(u8, String)>,
    ) -> Self {
        Self {
            translation_name,
            passage_indexes: (from, to),
            verses,
        }
    }

    fn present(&self) -> Element<crate::Message> {
        todo!()
    }
}

/// Jeden slajd při promítání písně
struct SongSlide {
    /// Název písně
    title: String,
    /// Obsah dané části písně
    content: String,
}

impl SongSlide {
    fn new(title: String, content: String) -> Self {
        Self { title, content }
    }

    fn present(&self) -> Element<crate::Message> {
        todo!()
    }
}

/// Aby bylo možné globálně změnit prezentaci (začernit, zmrazit)
enum PresenterMode {
    /// Normální prezentace
    Normal,
    /// Prázdný snímek
    Blank,
}

pub enum Message {
    /// Přepne prezentaci na slajd s daným indexem
    SelectSlide(usize),
    /// Ukončí prezentaci
    EndPresentation,
}

pub struct Presenter {
    /// Prezentovaný playlist
    playlist_slides: Vec<Slide>,
    /// Index aktuálně prezentované položky
    current_presented_index: usize,
    /// Režim prezentace
    mode: PresenterMode,
}

/// Přetvoří `playlist` na vektor slajdů složený z položek vytvořených z jednotlivých
/// položek playlistu ve stejném pořadí.
fn playlist_to_slides(playlist: Playlist, verses_per_slide: usize) -> Vec<Slide> {
    let items = playlist.into_items();
    let slides: Vec<Slide> = items
        .into_iter()
        .flat_map(|item| match item {
            PlaylistItem::BiblePassage(passage) => {
                let name = passage.get_translation_name();
                let (from, to) = passage.get_range();
                passage
                    .get_verses()
                    .chunks(verses_per_slide)
                    .map(|verses| {
                        Slide::Passage(PassageSlide::new(
                            name.to_string(),
                            from,
                            to,
                            verses.to_vec(),
                        ))
                    })
                    .collect::<Vec<Slide>>()
            }
            PlaylistItem::Song(song) => {
                let title = song.title;
                song.order
                    .into_iter()
                    .map(|part_name| {
                        let part_content = song
                            .parts
                            .get(&part_name)
                            .expect("Píseň musí obsahovat všechny svoje části");
                        Slide::Song(SongSlide::new(title.clone(), part_content.to_string()))
                    })
                    .collect()
            }
        })
        .collect();

    slides
}

impl Presenter {
    /// Vytvoří nový `Presenter`. Playlist musí obsahovat alespoň jednu položku,
    /// jinak není co prezentovat a funkce vrátí Error.
    pub fn try_new(playlist: Playlist) -> Result<Presenter> {
        if playlist.items.is_empty() {
            Err(anyhow!("Nelze prezentovat prázdný playlist"))
        } else {
            Ok(Presenter {
                playlist_slides: playlist_to_slides(playlist, VERSES_PER_SLIDE),
                current_presented_index: 0,
                mode: PresenterMode::Normal,
            })
        }
    }
}
