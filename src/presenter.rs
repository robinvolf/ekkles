use anyhow::{Context, Result, anyhow};
use ekkles_data::playlist::PlaylistItem;
use ekkles_data::{bible::indexing::VerseIndex, playlist::Playlist};
use iced::keyboard::{Key, key};
use iced::widget::button::danger;
use iced::widget::{Space, button, column, container, radio, row, scrollable, slider, text};
use iced::window::{Id, Settings};
use iced::{Alignment, Color, Element, Length, Subscription, Task, Theme};
use log::{debug, trace};
use sqlx::Sqlite;
use sqlx::pool::PoolConnection;

use crate::components::playlist_item_styles;
use crate::pick_playlist::PlaylistPicker;
use crate::{Ekkles, Screen};

/// Počet veršů na jeden slajd, proteď konstanta
const VERSES_PER_SLIDE: usize = 2;

const TEXT_SIZE_MULTIPLIER_MIN: f32 = 0.5;
const TEXT_SIZE_MULTIPLIER_MAX: f32 = 3.0;
const TEXT_SIZE_MULTIPLIER_DEFAULT: f32 = 1.0;
/// Jelikož [`iced::widget::slider()`] potřebuje range a range přes f32 hodnoty se nechová dobře,
/// používám pro range u8 (0..=255) a pomocí [`normalize_text_multiplier`] range poté
/// normalizuji. Tato default hodnota by se měla promítnout do [`TEXT_SIZE_MULTIPLIER_DEFAULT`].
const TEXT_SIZE_MULTIPLIER_DEFAULT_U8: u8 = ((TEXT_SIZE_MULTIPLIER_DEFAULT
    - TEXT_SIZE_MULTIPLIER_MIN)
    / (TEXT_SIZE_MULTIPLIER_MAX - TEXT_SIZE_MULTIPLIER_MIN)
    * u8::MAX as f32) as u8;

/// Velikost textu pro hlavní obsah snímku
const MAIN_TEXT_SIZE: f32 = 70.0;
/// Velikost textu pro doplňující obsah snímku
const ADDITIONAL_TEXT_SIZE: f32 = 30.0;

// Poznámka: Musí to být malé písmena, jinak se nematchnou na keycode v subscription()
const MODE_FREEZE_KEY: &str = "f";
const MODE_NORMAL_KEY: &str = "n";
const MODE_BLANK_KEY: &str = "b";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Slide {
    Passage(PassageSlide),
    Song(SongSlide),
}

impl Slide {
    fn present(&self, text_size_multiplier: f32) -> Element<Message> {
        match self {
            Slide::Passage(passage_slide) => passage_slide.present(text_size_multiplier),
            Slide::Song(song_slide) => song_slide.present(text_size_multiplier),
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

    fn present(&self, text_size_multiplier: f32) -> Element<Message> {
        let verses_text_size = MAIN_TEXT_SIZE * text_size_multiplier;
        let indexes_text_size = ADDITIONAL_TEXT_SIZE * text_size_multiplier;

        let verses_text: String = self
            .verses
            .iter()
            .map(|(number, content)| format!("{}: {}", number, content))
            .collect();

        let indexes_text = format!("{} - {}", self.passage_indexes.0, self.passage_indexes.1);

        let verses = container(text(verses_text).size(verses_text_size)).center(Length::Fill);
        let indexes = container(
            text(indexes_text)
                .align_x(Alignment::Center)
                .size(indexes_text_size),
        )
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

    fn present(&self, text_size_multiplier: f32) -> Element<Message> {
        let content_size = MAIN_TEXT_SIZE * text_size_multiplier;
        let title_size = ADDITIONAL_TEXT_SIZE * text_size_multiplier;

        let content = container(
            text(&self.content)
                .align_x(Alignment::Center)
                .size(content_size),
        )
        .center(Length::Fill);

        let title = container(
            text(&self.title)
                .align_x(Alignment::Center)
                .size(title_size),
        )
        .center_x(Length::Fill)
        .align_bottom(Length::Shrink);

        container(column![content, title])
            .style(black_background)
            .into()
    }
}

/// Aby bylo možné globálně změnit prezentaci (začernit, zmrazit)
#[derive(Debug, Clone, Copy)]
pub enum PresentationMode {
    /// Normální prezentace
    Normal,
    /// Prázdný snímek
    Blank,
    /// Obrazovka zmražena na snímku s daným indexem
    Frozen(usize),
}

/// Ruční implementace [`PartialEq`] a [`Eq`], aby se v případě [`PresentationMode::Frozen`]
/// nekontrolovala shoda zabaleného indexu. Je to protože [`iced::widget::radio()`] podle `Eq`
/// rozeznává, zda-li je dané radio button zakliklé.
impl PartialEq for PresentationMode {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PresentationMode::Normal, PresentationMode::Normal) => true,
            (PresentationMode::Blank, PresentationMode::Blank) => true,
            (PresentationMode::Frozen(_), PresentationMode::Frozen(_)) => true,
            _ => false,
        }
    }
}
impl Eq for PresentationMode {}

#[derive(Clone, Debug)]
pub enum Message {
    /// Otevře prezentační okno
    OpenPresentationWindow,
    /// Prezentační okno bylo otevřeno pod daným ID
    PresentationWindowOpened(Id),
    /// Požaduje přepnutí prezentace na předchozí slajd
    RequestPrevSlide,
    /// Požaduje přepnutí prezentace na následující slajd
    RequestNextSlide,
    /// Přepne prezentaci na slajd s daným indexem
    SelectSlide(usize),
    /// Zavře prezentační okno
    ClosePresentationWindow,
    /// Prezentační okno je zavřeno
    PresentationWindowClosed,
    /// Změna módu prezentace
    PresentationModeChanged(PresentationMode),
    /// Zmrazit prezentaci, stejné jako PresentationModeChanged(PresentationMode::Frozen(_)),
    /// ale bez specifikace indexu. Nutné pro zamražení ze subscription.
    FreezePresentation,
    /// Změna multiplikátoru velikosti textu na snímku
    TextSizeMultiplierChanged(u8),
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
    mode: PresentationMode,
    /// Multiplikátor velikost textu na snímku, při použití se normalizuje do
    /// intervalu `[TEXT_SIZE_MULTIPLIER_MIN]` až [`TEXT_SIZE_MULTIPLIER_MAX`].
    /// Vysvětlení viz: [`TEXT_SIZE_MULTIPLIER_DEFAULT_U8`].
    text_scale: u8,
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
                text_scale: TEXT_SIZE_MULTIPLIER_DEFAULT_U8,
            })
        }
    }

    /// Vrátí odebírané subscriptions pro obrazovku Prezentér. Odebíráme vstupy od klávesnice.
    ///
    /// # Klávesy
    /// - Šipky ↑↓ pro posouvání právě promítané položky
    /// - Escape pro ukončení prezentace
    pub fn subscription(&self) -> Subscription<crate::Message> {
        iced::keyboard::on_key_press(|key, modifiers| {
            trace!("Přišel event z klávesnice: {:?}", (key.clone(), modifiers));
            match (key.as_ref(), modifiers) {
                (Key::Named(key::Named::ArrowUp), _) => Some(Message::RequestPrevSlide.into()),
                (Key::Named(key::Named::ArrowDown), _) => Some(Message::RequestNextSlide.into()),
                (Key::Named(key::Named::Escape), _) => {
                    Some(Message::ClosePresentationWindow.into())
                }
                (Key::Character(MODE_FREEZE_KEY), _) => Some(Message::FreezePresentation.into()),
                (Key::Character(MODE_NORMAL_KEY), _) => {
                    Some(Message::PresentationModeChanged(PresentationMode::Normal).into())
                }
                (Key::Character(MODE_BLANK_KEY), _) => {
                    Some(Message::PresentationModeChanged(PresentationMode::Blank).into())
                }
                _ => None,
            }
        })
    }

    pub fn get_presentation_window_id(&self) -> Option<Id> {
        self.presentation_window_id
    }

    fn is_first_slide_selected(&self) -> bool {
        self.current_presented_index == 0
    }

    fn is_last_slide_selected(&self) -> bool {
        self.current_presented_index == self.playlist_slides.len() - 1
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

        let first_slide_selected = self.is_first_slide_selected();
        let last_slide_selected = self.is_last_slide_selected();
        trace!(
            "Vybrán první slajd? [{first_slide_selected}] Vybrán poslední slajd? [{last_slide_selected}]"
        );

        let reset_text_size_button_msg = if self.text_scale == TEXT_SIZE_MULTIPLIER_DEFAULT_U8 {
            None
        } else {
            Some(Message::TextSizeMultiplierChanged(
                TEXT_SIZE_MULTIPLIER_DEFAULT_U8,
            ))
        };

        let style_control = column![
            radio(
                String::from("Normál (") + MODE_NORMAL_KEY + ")",
                PresentationMode::Normal,
                Some(self.mode),
                Message::PresentationModeChanged
            ),
            radio(
                String::from("Prázdný snímek (") + MODE_BLANK_KEY + ")",
                PresentationMode::Blank,
                Some(self.mode),
                Message::PresentationModeChanged
            ),
            radio(
                String::from("Zmrazit (") + MODE_FREEZE_KEY + ")",
                PresentationMode::Frozen(self.current_presented_index),
                Some(self.mode),
                Message::PresentationModeChanged
            ),
            Space::with_height(Length::Fixed(30.0)),
            text("Škálování velikosti textu"),
            row![
                slider(
                    u8::MIN..=u8::MAX,
                    self.text_scale,
                    Message::TextSizeMultiplierChanged
                ),
                button("Resetovat").on_press_maybe(reset_text_size_button_msg)
            ]
            .spacing(5)
            .align_y(Alignment::Center)
        ]
        .spacing(10)
        .padding(30);

        let presentation_control = column![
            button("Nahoru")
                .width(Length::Fill)
                .on_press_maybe(if first_slide_selected {
                    None
                } else {
                    Some(Message::RequestPrevSlide)
                }),
            button("Dolů")
                .width(Length::Fill)
                .on_press_maybe(if last_slide_selected {
                    None
                } else {
                    Some(Message::RequestNextSlide)
                }),
            Space::with_height(Length::Fixed(30.0)),
            button("Ukončit prezentaci (ESC)")
                .width(Length::Fill)
                .style(danger)
                .on_press(Message::ClosePresentationWindow),
        ]
        .spacing(10)
        .padding(30);

        Into::<Element<Message>>::into(container(
            row![
                presentation_control
                    .width(Length::FillPortion(1))
                    .height(Length::Fill),
                scrollable(column(slide_list).spacing(5).align_x(Alignment::Center))
                    .width(Length::FillPortion(2))
                    .height(Length::Fill),
                style_control
                    .width(Length::FillPortion(1))
                    .height(Length::Fill)
            ]
            .padding(10)
            .height(Length::Fill)
            .align_y(Alignment::Center),
        ))
    }

    /// Zkonstruuuje GUI pro prezentační okno
    pub fn view_presentation(&self) -> Element<Message> {
        let text_size_multiplier = normalize_text_multiplier(self.text_scale);

        match self.mode {
            PresentationMode::Normal => {
                self.playlist_slides[self.current_presented_index].present(text_size_multiplier)
            }
            PresentationMode::Blank => blank_slide(),
            PresentationMode::Frozen(frozen_index) => {
                self.playlist_slides[frozen_index].present(text_size_multiplier)
            }
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
                let (id, task) = iced::window::open(Settings {
                    fullscreen: true,
                    ..Settings::default()
                });
                presenter.presentation_window_id = Some(id);
                task.map(|id| Message::PresentationWindowOpened(id).into())
            }
            Message::PresentationWindowOpened(id) => {
                debug!("Prezentační okno otevřeno pod id {id}");
                presenter.presentation_window_id = Some(id);
                Task::none()
            }
            Message::PresentationModeChanged(presentation_mode) => {
                debug!("Nastavuji prezentační režim na {:?}", presentation_mode);
                presenter.mode = presentation_mode;
                Task::none()
            }
            Message::TextSizeMultiplierChanged(multiplier) => {
                debug!("Nastavuji multiplikátor velikosti textu na {multiplier}");
                presenter.text_scale = multiplier;
                Task::none()
            }
            Message::RequestPrevSlide => {
                debug!("Požadavek k přechodu na předchozí slajd");
                if presenter.is_first_slide_selected() {
                    Task::none()
                } else {
                    let new_slide_index = presenter.current_presented_index - 1;
                    Task::done(Message::SelectSlide(new_slide_index).into())
                }
            }
            Message::RequestNextSlide => {
                debug!("Požadavek k přechodu na následující slajd");
                if presenter.is_last_slide_selected() {
                    Task::none()
                } else {
                    let new_slide_index = presenter.current_presented_index + 1;
                    Task::done(Message::SelectSlide(new_slide_index).into())
                }
            }
            Message::FreezePresentation => {
                let current_index = presenter.current_presented_index;
                debug!("Zamražuji prezentaci na indexu {current_index}");
                Task::done(
                    Message::PresentationModeChanged(PresentationMode::Frozen(current_index))
                        .into(),
                )
            }
        }
    }
}

/// Normalizuje pomocí lineární transformace multiplikátor textu o hodnotě `value` tak,
/// aby platilo:
/// ```rust
/// assert_eq!(normalize_text_multiplier(0), TEXT_SIZE_MULTIPLIER_MIN);
/// assert_eq!(normalize_text_multiplier(255), TEXT_SIZE_MULTIPLIER_MAX);
/// assert_eq!(normalize_text_multiplier(TEXT_SIZE_MULTIPLIER_DEFAULT_U8), TEXT_SIZE_MULTIPLIER_DEFAULT);
/// ```
fn normalize_text_multiplier(value: u8) -> f32 {
    let value: f32 = value.into();

    let min: f32 = u8::MIN.into();
    let max: f32 = u8::MAX.into();

    let zero_to_one = (value - min) / max;

    zero_to_one * (TEXT_SIZE_MULTIPLIER_MAX - TEXT_SIZE_MULTIPLIER_MIN) + TEXT_SIZE_MULTIPLIER_MIN
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
