use anyhow::{Context, Result, anyhow};
use ekkles_data::playlist::{PlaylistItem, PlaylistMetadata};
use ekkles_data::{bible::indexing::VerseIndex, playlist::Playlist};
use iced::widget::button::danger;
use iced::widget::{Space, Text, button, column, container, radio, row, scrollable, text};
use iced::window::{Id, Settings};
use iced::{Alignment, Color, Element, Length, Task, Theme};
use log::debug;
use sqlx::Sqlite;
use sqlx::pool::PoolConnection;

use crate::components::playlist_item_styles;
use crate::pick_playlist::PlaylistPicker;
use crate::{Ekkles, Screen};

/// Počet veršů na jeden slajd, proteď konstanta
const VERSES_PER_SLIDE: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Slide {
    Passage(PassageSlide),
    Song(SongSlide),
}

impl Slide {
    fn present(&self) -> Element<Message> {
        match self {
            Slide::Passage(passage_slide) => passage_slide.present(),
            Slide::Song(song_slide) => song_slide.present(),
        }
    }
}

/// Jeden slajd při promítání pasáže
#[derive(Debug, Clone, PartialEq, Eq)]
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

    fn present(&self) -> Element<Message> {
        let verses_text: String = self
            .verses
            .iter()
            .map(|(number, content)| format!("{}: {}", number, content))
            .collect();

        let indexes_text = format!("{} - {}", self.passage_indexes.0, self.passage_indexes.1);

        let verses = container(text(verses_text).size(50))
            .center(Length::Fill)
            .padding(30);
        let indexes = container(text(indexes_text).align_x(Alignment::Center).size(30))
            .padding(30)
            .center_x(Length::Fill)
            .align_bottom(Length::Shrink);

        container(column![verses, indexes])
            .style(black_background)
            .into()
    }
}

/// Jeden slajd při promítání písně
#[derive(Debug, Clone, PartialEq, Eq)]
struct SongSlide {
    /// Název písně
    title: String,
    /// Název části písně
    part_name: String,
    /// Obsah dané části písně
    content: String,
}

impl SongSlide {
    fn new(title: String, part_name: String, content: String) -> Self {
        Self {
            title,
            part_name,
            content,
        }
    }

    fn present(&self) -> Element<Message> {
        let content = container(text(&self.content).align_x(Alignment::Center).size(50))
            .center(Length::Fill)
            .padding(30);
        let title = container(text(&self.title).align_x(Alignment::Center).size(30))
            .center_x(Length::Fill)
            .align_bottom(Length::Shrink)
            .padding(30);

        container(column![content, title])
            .style(black_background)
            .into()
    }
}

/// Aby bylo možné globálně změnit prezentaci (začernit, zmrazit)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PresentationMode {
    /// Normální prezentace
    Normal,
    /// Prázdný snímek
    Blank,
    /// Obrazovka zmražena na snímku s daným indexem
    Frozen(usize),
}

#[derive(Clone, Debug)]
pub enum Message {
    /// Otevře prezentační okno
    OpenPresentationWindow,
    /// Prezentační okno bylo otevřeno pod daným ID
    PresentationWindowOpened(Id),
    /// Přepne prezentaci na slajd s daným indexem
    SelectSlide(usize),
    /// Zavře prezentační okno
    ClosePresentationWindow,
    /// Prezentační okno je zavřeno
    PresentationWindowClosed,
    PressentationModeChanged(PresentationMode),
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::Presenter(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presenter {
    /// Id okna s prezentací
    presentation_window_id: Option<Id>,
    /// Prezentovaný playlist
    playlist_slides: Vec<Slide>,
    /// Index aktuálně prezentované položky
    current_presented_index: usize,
    /// Režim prezentace
    mode: PresentationMode,
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
                        Slide::Song(SongSlide::new(
                            title.clone(),
                            part_name,
                            part_content.to_string(),
                        ))
                    })
                    .collect()
            }
        })
        .collect();

    slides
}

impl Presenter {
    pub fn get_window_id(&self) -> Option<Id> {
        self.presentation_window_id
    }

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
                mode: PresentationMode::Normal,
                presentation_window_id: None,
            })
        }
    }

    pub fn get_presentation_window_id(&self) -> Option<Id> {
        self.presentation_window_id
    }

    /// Zkonstruuje GUI pro ovládací okno
    pub fn view_control(&self) -> Element<Message> {
        // Na několika místech se musí explicitně specifikovat typ, protože automatická
        // inference typů shoří kvůli ukazateli na funkci
        type MsgAndStyle = (
            Option<Message>,
            fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style,
        );

        let slide_list =
            self.playlist_slides
                .iter()
                .enumerate()
                .map(|(index, slide)| match slide {
                    Slide::Passage(slide) => {
                        let (from, to) = slide.passage_indexes;
                        let (maybe_msg, style): MsgAndStyle =
                            if index == self.current_presented_index {
                                (None, playlist_item_styles::passage_selected)
                            } else {
                                (
                                    Some(Message::SelectSlide(index)),
                                    playlist_item_styles::passage,
                                )
                            };
                        button(text!("Pasáž {} - {}", from, to))
                            .width(Length::Fill)
                            .on_press_maybe(maybe_msg)
                            .style(style)
                            .into()
                    }
                    Slide::Song(slide) => {
                        let title = &slide.title;
                        let part_name = &slide.part_name;
                        let (maybe_msg, style): MsgAndStyle =
                            if index == self.current_presented_index {
                                (None, playlist_item_styles::song_selected)
                            } else {
                                (
                                    Some(Message::SelectSlide(index)),
                                    playlist_item_styles::song,
                                )
                            };
                        button(text!("Píseň {}: {}", title, part_name))
                            .width(Length::Fill)
                            .on_press_maybe(maybe_msg)
                            .style(style)
                            .into()
                    }
                });

        let top_selected = self.current_presented_index == 0;
        let bottom_selected =
            self.playlist_slides.get(self.current_presented_index) == self.playlist_slides.last();

        let control_buttons = column![
            button("Nahoru")
                .width(Length::Fill)
                .on_press_maybe(if top_selected {
                    None
                } else {
                    Some(Message::SelectSlide(self.current_presented_index - 1))
                }),
            button("Dolů")
                .width(Length::Fill)
                .on_press_maybe(if bottom_selected {
                    None
                } else {
                    Some(Message::SelectSlide(self.current_presented_index + 1))
                }),
            Space::with_height(Length::Fixed(30.0)),
            row![
                radio(
                    "Normál",
                    PresentationMode::Normal,
                    if let PresentationMode::Normal = self.mode {
                        Some(PresentationMode::Normal)
                    } else {
                        None
                    },
                    Message::PressentationModeChanged
                ),
                radio(
                    "Prázdný snímek",
                    PresentationMode::Blank,
                    if let PresentationMode::Blank = self.mode {
                        Some(PresentationMode::Blank)
                    } else {
                        None
                    },
                    Message::PressentationModeChanged
                ),
                radio(
                    "Zmrazit",
                    PresentationMode::Frozen(self.current_presented_index),
                    if let PresentationMode::Frozen(index) = self.mode {
                        Some(PresentationMode::Frozen(index))
                    } else {
                        None
                    },
                    Message::PressentationModeChanged
                ),
            ]
            .spacing(10),
            Space::with_height(Length::Fixed(30.0)),
            button("Ukončit prezentaci")
                .width(Length::Fill)
                .style(danger)
                .on_press(Message::ClosePresentationWindow),
        ]
        .spacing(10)
        .height(Length::Fill)
        .width(Length::FillPortion(1))
        .padding(30);

        Into::<Element<Message>>::into(container(
            row![
                control_buttons,
                scrollable(column(slide_list).spacing(5).align_x(Alignment::Center))
                    .width(Length::FillPortion(2))
                    .height(Length::Fill),
                column([]).width(Length::FillPortion(1))
            ]
            .padding(10)
            .height(Length::Fill)
            .align_y(Alignment::Center),
        ))
    }

    /// Zkonstruuuje GUI pro prezentační okno
    pub fn view_presentation(&self) -> Element<Message> {
        match self.mode {
            PresentationMode::Normal => {
                self.playlist_slides[self.current_presented_index].present()
            }
            PresentationMode::Blank => blank_slide(),
            PresentationMode::Frozen(frozen_index) => self.playlist_slides[frozen_index].present(),
        }
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
            Message::ClosePresentationWindow => {
                debug!("Ukončuji prezentaci, vracím se na seznam playlistů");
                iced::window::close(
                    presenter
                        .presentation_window_id
                        .expect("Nelze zavřít prezentační okno, pokud nebylo otevřeno"),
                )
                .chain(Task::done(Message::PresentationWindowClosed.into()))
            }
            Message::PresentationWindowClosed => {
                state.screen = Screen::PickPlaylist(PlaylistPicker::new());
                Task::done(crate::pick_playlist::Message::LoadPlaylists.into())
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
            Message::PressentationModeChanged(presentation_mode) => {
                debug!("Nastavuji prezentační režim na {:?}", presentation_mode);
                presenter.mode = presentation_mode;
                Task::none()
            }
        }
    }
}

/// Vytvoří prázdný slide
fn blank_slide() -> Element<'static, Message> {
    container(Space::new(Length::Fill, Length::Fill))
        .style(black_background)
        .into()
}

/// Stylovací funkce pro pozadí slajdu
fn black_background(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(Color::WHITE),
        background: Some(iced::Background::Color(Color::BLACK)),
        ..Default::default()
    }
}
