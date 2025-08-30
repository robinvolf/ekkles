use anyhow::{Context, Result, anyhow};
use ekkles_data::playlist::{PlaylistItem, PlaylistMetadata};
use ekkles_data::{bible::indexing::VerseIndex, playlist::Playlist};
use iced::widget::{Text, text};
use iced::window::{Id, Settings};
use iced::{Element, Task};
use log::debug;
use sqlx::Sqlite;
use sqlx::pool::PoolConnection;

use crate::pick_playlist::PlaylistPicker;
use crate::{Ekkles, Screen};

/// Počet veršů na jeden slajd, proteď konstanta
const VERSES_PER_SLIDE: usize = 2;

#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
enum PresenterMode {
    /// Normální prezentace
    Normal,
    /// Prázdný snímek
    Blank,
}

#[derive(Clone, Debug)]
pub enum Message {
    /// Otevře prezentační okno
    OpenPresentationWindow,
    /// Prezentační okno bylo otevřeno pod daným ID
    PresentationWindowOpened(Id),
    /// Přepne prezentaci na slajd s daným indexem
    SelectSlide(usize),
    /// Ukončí prezentaci
    EndPresentation,
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::Presenter(value)
    }
}

#[derive(Debug, Clone)]
pub struct Presenter {
    /// Id okna s prezentací
    presentation_window_id: Option<Id>,
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
    pub async fn try_new(playlist_id: i64, conn: &mut PoolConnection<Sqlite>) -> Result<Presenter> {
        let playlist = Playlist::load(playlist_id, conn)
            .await
            .context("Nelze načíst playlist z databáze")?;

        if playlist.items.is_empty() {
            Err(anyhow!("Nelze prezentovat prázdný playlist"))
        } else {
            Ok(Presenter {
                playlist_slides: playlist_to_slides(playlist, VERSES_PER_SLIDE),
                current_presented_index: 0,
                mode: PresenterMode::Normal,
                presentation_window_id: None,
            })
        }
    }

    pub fn get_presentation_window_id(&self) -> Option<Id> {
        self.presentation_window_id
    }

    /// Zkonstruuje GUI pro ovládací okno
    pub fn view_control(&self) -> Element<Message> {
        text("Tady bude ovládání prezentace").into()
    }

    /// Zkonstruuuje GUI pro prezentační okno
    pub fn view_presentation(&self) -> Element<Message> {
        text("Tady bude prezentace").into()
    }

    pub fn update(state: &mut Ekkles, msg: Message) -> Task<crate::Message> {
        let presenter = match &mut state.screen {
            crate::Screen::Presenter(presenter) => presenter,
            screen => panic!("Update pro Presenter zavolán na obrazove: {:?}", screen),
        };

        match msg {
            Message::SelectSlide(index) => {
                debug!("Vybírám slajd s indexem {index}");
                presenter.current_presented_index = index;
                Task::none()
            }
            Message::EndPresentation => {
                todo!()
                // debug!("Ukončuji prezentaci, vracím se na seznam playlistů");
                // state.screen = Screen::PickPlaylist(PlaylistPicker::new());
                // Task::done(crate::pick_playlist::Message::LoadPlaylists.into())
            }
            Message::OpenPresentationWindow => {
                debug!("Otevírám prezentační okno");
                let (id, task) = iced::window::open(Settings::default());
                presenter.presentation_window_id = Some(id);
                task.map(|id| Message::PresentationWindowOpened(id).into())
            }
            Message::PresentationWindowOpened(id) => {
                debug!("Prezentační okno otevřeno pod id {id}");
                presenter.presentation_window_id = Some(id);
                Task::none()
            }
        }
    }
}
