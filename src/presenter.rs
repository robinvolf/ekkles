use anyhow::{Context, Result, anyhow};
use ekkles_data::bible::indexing::VerseIndices;
use ekkles_data::playlist::PlaylistItem;
use ekkles_data::{bible::indexing::VerseIndex, playlist::Playlist};
use iced::Length::FillPortion;
use iced::keyboard::{Key, Modifiers, key};
use iced::widget::button::danger;
use iced::widget::{button, column, container, radio, row, scrollable, slider, space, text};
use iced::window::{Id, Settings};
use iced::{Alignment, Color, Element, Length, Subscription, Task, Theme};
use log::{debug, error, trace};
use sqlx::Sqlite;
use sqlx::pool::PoolConnection;

use crate::components::playlist_item_styles;
use crate::components::shortcuts::KeyboardShortcut;
use crate::start_screen::StartScreen;
use crate::{Ekkles, Screen};

/// Počet veršů na jeden slajd, proteď konstanta
const VERSES_PER_SLIDE: usize = 1;

const TEXT_SIZE_MULTIPLIER_MIN: f32 = 0.3;
const TEXT_SIZE_MULTIPLIER_MAX: f32 = 2.0;
const TEXT_SIZE_MULTIPLIER_DEFAULT: f32 = 1.0;
/// Jelikož [`iced::widget::slider()`] potřebuje range a range přes f32 hodnoty se nechová dobře,
/// používám pro range u8 (0..=255) a pomocí [`normalize_text_multiplier`] range poté
/// normalizuji. Tato default hodnota by se měla promítnout do [`TEXT_SIZE_MULTIPLIER_DEFAULT`].
const TEXT_SIZE_MULTIPLIER_DEFAULT_U8: u8 = ((TEXT_SIZE_MULTIPLIER_DEFAULT
    - TEXT_SIZE_MULTIPLIER_MIN)
    / (TEXT_SIZE_MULTIPLIER_MAX - TEXT_SIZE_MULTIPLIER_MIN)
    * u8::MAX as f32) as u8;

/// Velikost textu pro hlavní obsah snímku
const MAIN_TEXT_SIZE: f32 = 50.0;
/// Velikost textu pro doplňující obsah snímku
const ADDITIONAL_TEXT_SIZE: f32 = 30.0;

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

        let verses_content: String = self
            .verses
            .iter()
            .map(|(number, content)| format!("{}: {}", number, content))
            .collect();

        let indexes_content = format!("{} - {}", self.passage_indexes.0, self.passage_indexes.1);

        let verses = container(
            text(verses_content)
                .size(verses_text_size)
                .wrapping(text::Wrapping::WordOrGlyph), // Abychom věděli, kdy změnit velikost textu
        )
        .center(Length::Fill);
        let indexes = container(
            text(indexes_content)
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
    /// Obrazovka zmražena na položce `item` na slajdu položky `item_slide`
    Frozen { item: usize, item_slide: usize },
}

/// Ruční implementace [`PartialEq`] a [`Eq`], aby se v případě [`PresentationMode::Frozen`]
/// nekontrolovala shoda zabaleného indexu. Je to protože [`iced::widget::radio()`] podle `Eq`
/// rozeznává, zda-li je dané radio button zakliklé.
impl PartialEq for PresentationMode {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PresentationMode::Normal, PresentationMode::Normal) => true,
            (PresentationMode::Blank, PresentationMode::Blank) => true,
            (PresentationMode::Frozen { .. }, PresentationMode::Frozen { .. }) => true,
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
    PrevSlide,
    /// Požaduje přepnutí prezentace na následující slajd
    NextSlide,
    /// Přepne prezentaci na položku `item` na slajd `slide`
    SelectSlide { item: usize, slide: usize },
    /// Zavře prezentační okno
    ClosePresentationWindow,
    /// Prezentační okno je zavřeno
    PresentationWindowClosed,
    /// Nastavit na normální výstup
    NormalOuput,
    /// Zmrazí výstup
    FreezeOuput,
    /// Nastaví výstup na černou obrazovku
    BlankOuput,
    /// Změna multiplikátoru velikosti textu na snímku
    TextSizeMultiplierChanged(u8),
}

impl From<KeyboardShortcutMessage> for Message {
    fn from(value: KeyboardShortcutMessage) -> Self {
        match value {
            KeyboardShortcutMessage::NextSlide => Message::NextSlide,
            KeyboardShortcutMessage::PrevSlide => Message::PrevSlide,
            KeyboardShortcutMessage::ClosePresentation => Message::ClosePresentationWindow,
            KeyboardShortcutMessage::NormalOuput => Message::NormalOuput,
            KeyboardShortcutMessage::FreezeOuput => Message::FreezeOuput,
            KeyboardShortcutMessage::BlankOuput => Message::BlankOuput,
        }
    }
}

#[derive(Debug, Clone, Copy, Hash)]
enum KeyboardShortcutMessage {
    NextSlide,
    PrevSlide,
    ClosePresentation,
    NormalOuput,
    FreezeOuput,
    BlankOuput,
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
    playlist: Playlist,
    /// Index aktuálně prezentované položky playlistu
    item_index: usize,
    /// Index slajdu prezentované položky. Každá položka playlistu může být rozbalena do
    /// `n` slajdů. Toto je index v intervalu `0..n`.
    item_slide_index: usize,
    /// Režim prezentace
    mode: PresentationMode,
    /// Multiplikátor velikost textu na snímku, při použití se normalizuje do
    /// intervalu `[TEXT_SIZE_MULTIPLIER_MIN]` až [`TEXT_SIZE_MULTIPLIER_MAX`].
    /// Vysvětlení viz: [`TEXT_SIZE_MULTIPLIER_DEFAULT_U8`].
    text_scale: u8,
    /// Klávesové zkratky
    shortcuts: [KeyboardShortcut<KeyboardShortcutMessage>; 6],
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
                playlist,
                item_index: 0,
                item_slide_index: 0,
                mode: PresentationMode::Normal,
                presentation_window_id: None,
                text_scale: TEXT_SIZE_MULTIPLIER_DEFAULT_U8,
                shortcuts: [
                    KeyboardShortcut::new(
                        Key::Named(key::Named::ArrowUp),
                        Modifiers::empty(),
                        KeyboardShortcutMessage::PrevSlide,
                        "Předchozí slajd",
                    ),
                    KeyboardShortcut::new(
                        Key::Named(key::Named::ArrowDown),
                        Modifiers::empty(),
                        KeyboardShortcutMessage::NextSlide,
                        "Následující slajd",
                    ),
                    KeyboardShortcut::new(
                        Key::Named(key::Named::Escape),
                        Modifiers::empty(),
                        KeyboardShortcutMessage::ClosePresentation,
                        "Ukonči prezentaci",
                    ),
                    KeyboardShortcut::new(
                        Key::Character("f".into()),
                        Modifiers::empty(),
                        KeyboardShortcutMessage::FreezeOuput,
                        "Zmrazit, výstup",
                    ),
                    KeyboardShortcut::new(
                        Key::Character("n".into()),
                        Modifiers::empty(),
                        KeyboardShortcutMessage::NormalOuput,
                        "Normální, výstup",
                    ),
                    KeyboardShortcut::new(
                        Key::Character("b".into()),
                        Modifiers::empty(),
                        KeyboardShortcutMessage::BlankOuput,
                        "Prázdný výstup",
                    ),
                ],
            })
        }
    }

    pub fn subscription(&self) -> Subscription<crate::Message> {
        KeyboardShortcut::subscription(self.shortcuts.clone())
            .map(Message::from)
            .map(crate::Message::from)
    }

    pub fn get_presentation_window_id(&self) -> Option<Id> {
        self.presentation_window_id
    }

    fn is_first_slide_selected(&self) -> bool {
        self.item_index == 0 && self.item_slide_index == 0
    }

    fn is_last_slide_selected(&self) -> bool {
        if self.playlist.items.is_empty() {
            error!("Prázdný playlist v prezentaci");
            true
        } else {
            let last_item_selected = self.item_index == self.playlist.items.len() - 1;
            let last_slide_of_last_item_selected = self.item_slide_index
                == num_slides(
                    &self.playlist.items[self.playlist.items.len() - 1],
                    VERSES_PER_SLIDE,
                ) - 1;

            last_item_selected && last_slide_of_last_item_selected
        }
    }

    /// Zkonstruuje GUI pro ovládací okno
    pub fn view_control(&self) -> Element<Message> {
        // Na několika místech se musí explicitně specifikovat typ, protože automatická
        // inference typů shoří kvůli ukazateli na funkci
        type MsgAndStyle = (
            Option<Message>,
            fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style,
        );

        // let slide_list =
        //     self.playlist_slides
        //         .iter()
        //         .enumerate()
        //         .map(|(index, slide)| match slide {
        //             Slide::Passage(slide) => {
        //                 let (from, to) = slide.passage_indexes;
        //                 let (maybe_msg, style): MsgAndStyle =
        //                     if index == self.current_presented_index {
        //                         (None, playlist_item_styles::passage_selected)
        //                     } else {
        //                         (
        //                             Some(Message::SelectSlide(index)),
        //                             playlist_item_styles::passage,
        //                         )
        //                     };
        //                 button(text!("Pasáž {} - {}", from, to))
        //                     .width(Length::Fill)
        //                     .on_press_maybe(maybe_msg)
        //                     .style(style)
        //                     .into()
        //             }
        //             Slide::Song(slide) => {
        //                 let title = &slide.title;
        //                 let part_name = &slide.part_name;
        //                 let (maybe_msg, style): MsgAndStyle =
        //                     if index == self.current_presented_index {
        //                         (None, playlist_item_styles::song_selected)
        //                     } else {
        //                         (
        //                             Some(Message::SelectSlide(index)),
        //                             playlist_item_styles::song,
        //                         )
        //                     };
        //                 button(text!("Píseň {}: {}", title, part_name))
        //                     .width(Length::Fill)
        //                     .on_press_maybe(maybe_msg)
        //                     .style(style)
        //                     .into()
        //             }
        //         });

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
            radio("Normál", PresentationMode::Normal, Some(self.mode), |_| {
                Message::NormalOuput
            }),
            radio(
                "Prázdný snímek",
                PresentationMode::Blank,
                Some(self.mode),
                |_| { Message::BlankOuput }
            ),
            radio(
                "Zmrazit",
                PresentationMode::Frozen {
                    item: self.item_index,
                    item_slide: self.item_slide_index
                },
                Some(self.mode),
                |_| { Message::FreezeOuput }
            ),
            space().height(Length::Fixed(30.0)),
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
        .spacing(10);

        let presentation_control = column![
            button("Nahoru")
                .width(Length::Fill)
                .on_press_maybe(if first_slide_selected {
                    None
                } else {
                    Some(Message::PrevSlide)
                }),
            button("Dolů")
                .width(Length::Fill)
                .on_press_maybe(if last_slide_selected {
                    None
                } else {
                    Some(Message::NextSlide)
                }),
            space().height(Length::Fixed(30.0)),
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
                column![
                    // scrollable(column(slide_list).spacing(5).align_x(Alignment::Center))
                    //     .height(Length::Fill),
                    // preview,
                ]
                .width(Length::FillPortion(2)),
                column![
                    container(
                        style_control
                            .width(Length::FillPortion(1))
                            .height(Length::Fill)
                    )
                    .align_top(FillPortion(1)),
                    KeyboardShortcut::view(&self.shortcuts),
                ]
                .padding(30),
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
                // self.playlist_slides[self.current_presented_index].present(text_size_multiplier)
                render_slide(
                    &self.playlist.items,
                    self.item_index,
                    self.item_slide_index,
                    text_size_multiplier,
                )
            }
            PresentationMode::Blank => blank_slide(),
            PresentationMode::Frozen { item, item_slide } => {
                render_slide(&self.playlist.items, item, item_slide, text_size_multiplier)
            }
        }
    }

    pub fn update(state: &mut Ekkles, msg: Message) -> Task<crate::Message> {
        let presenter = match &mut state.screen {
            crate::Screen::Presenter(presenter) => presenter,
            screen => panic!("Update pro Presenter zavolán na obrazove: {:?}", screen),
        };

        match msg {
            Message::SelectSlide { item, slide } => {
                debug!("Vybírám položku indexem {item}, se slajdem {slide}");
                presenter.item_index = item;
                presenter.item_slide_index = slide;
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
                state.screen = Screen::StartScreen(StartScreen::new());
                Task::none()
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
            Message::TextSizeMultiplierChanged(multiplier) => {
                debug!("Nastavuji multiplikátor velikosti textu na {multiplier}");
                presenter.text_scale = multiplier;
                Task::none()
            }
            Message::PrevSlide => {
                debug!("Požadavek k přechodu na předchozí slajd");
                if presenter.is_first_slide_selected() {
                    Task::none()
                } else {
                    let (item, slide) = if presenter.item_slide_index == 0 {
                        let prev_item_slides_num = num_slides(
                            &presenter.playlist.items[presenter.item_index - 1],
                            VERSES_PER_SLIDE,
                        );

                        (presenter.item_index - 1, prev_item_slides_num - 1)
                    } else {
                        (presenter.item_index, presenter.item_slide_index - 1)
                    };
                    Task::done(Message::SelectSlide { item, slide }.into())
                }
            }
            Message::NextSlide => {
                debug!("Požadavek k přechodu na následující slajd");
                if presenter.is_last_slide_selected() {
                    Task::none()
                } else {
                    let curr_item_slides_num = num_slides(
                        &presenter.playlist.items[presenter.item_index],
                        VERSES_PER_SLIDE,
                    );

                    let (item, slide) = if presenter.item_slide_index == curr_item_slides_num - 1 {
                        (presenter.item_index + 1, 0)
                    } else {
                        (presenter.item_index, presenter.item_slide_index + 1)
                    };
                    Task::done(Message::SelectSlide { item, slide }.into())
                }
            }
            Message::FreezeOuput => {
                let item_index = presenter.item_index;
                let slide_index = presenter.item_slide_index;
                debug!("Zamražuji prezentaci na indexu {item_index}:{slide_index}");
                presenter.mode = PresentationMode::Frozen {
                    item: item_index,
                    item_slide: slide_index,
                };
                Task::none()
            }
            Message::NormalOuput => {
                debug!("Nastavuji prezentační výstup na normální");
                presenter.mode = PresentationMode::Normal;
                Task::none()
            }
            Message::BlankOuput => {
                debug!("Nastavuji prezentační výstup na prázdný snímek");
                presenter.mode = PresentationMode::Blank;
                Task::none()
            }
        }
    }
}

/// Vytvoří grafickou reprezentaci `item_slide_index`-tého slajdu `item_index`-té položky.
/// Pokud je jeden z indexů neplatný, zpanikaří.
///
/// Text Bude Škálován pomocí `scale`.
fn render_slide(
    items: &[PlaylistItem],
    item_index: usize,
    item_slide_index: usize,
    scale: f32,
) -> Element<Message> {
    let item = &items[item_index];

    match item {
        PlaylistItem::BiblePassage(passage) => {
            let verses_text_size = MAIN_TEXT_SIZE * scale;
            let indexes_text_size = ADDITIONAL_TEXT_SIZE * scale;

            let verses_content: String = passage
                .get_verses()
                .iter()
                .map(|(number, content)| format!("{}: {}", number, content))
                .collect();

            let indices: VerseIndices = passage.get_range().into();
            let indices_content = indices.to_string();

            let verses = container(
                text(verses_content)
                    .size(verses_text_size)
                    .wrapping(text::Wrapping::WordOrGlyph), // Abychom věděli, kdy změnit velikost textu
            )
            .center(Length::Fill);
            let indexes = container(
                text(indices_content)
                    .align_x(Alignment::Center)
                    .size(indexes_text_size),
            )
            .center_x(Length::Fill)
            .align_bottom(Length::Shrink);

            container(column![verses, indexes])
                .style(black_background)
                .into()
        }
        PlaylistItem::Song(song) => {
            let content_size = MAIN_TEXT_SIZE * scale;
            let title_size = ADDITIONAL_TEXT_SIZE * scale;

            let part_index = &song.order[item_slide_index];
            let content = &song.parts[part_index];
            let title = &song.title;

            let content = container(text(content).align_x(Alignment::Center).size(content_size))
                .center(Length::Fill);

            let title = container(text(title).align_x(Alignment::Center).size(title_size))
                .center_x(Length::Fill)
                .align_bottom(Length::Shrink);

            container(column![content, title])
                .style(black_background)
                .into()
        }
    }
}

/// Spočítá počet slajdů, kolik `item` zabere.
fn num_slides(item: &PlaylistItem, verses_per_slide: usize) -> usize {
    match item {
        PlaylistItem::BiblePassage(passage) => {
            let num_verses = passage.get_verses().iter().count();
            num_verses / verses_per_slide + num_verses % verses_per_slide
        }
        PlaylistItem::Song(song) => song.parts.len(),
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
    container(space().height(Length::Fill).width(Length::Fill))
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
