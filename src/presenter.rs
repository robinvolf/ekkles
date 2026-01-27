use std::iter;

use anyhow::Context;
use anyhow::{Result, anyhow};
use ekkles_data::Song;
use ekkles_data::bible::indexing::{Passage, VerseIndices};
use ekkles_data::playlist::Playlist;
use ekkles_data::playlist::PlaylistItem;
use iced::Length::FillPortion;
use iced::keyboard::{Key, Modifiers, key};
use iced::widget::button::danger;
use iced::widget::responsive;
use iced::widget::text::LineHeight;
use iced::widget::{button, column, container, radio, row, scrollable, slider, space, table, text};
use iced::window;
use iced::window::{Id, Settings};
use iced::{Alignment, Color, Element, Length, Pixels, Size, Subscription, Task, Theme};
use log::{debug, error, trace};

use crate::components::OpenedPicker;
use crate::components::bible_picker::{self, BiblePicker};
use crate::components::playlist_item_styles::{self, playlist_item_button_style2};
use crate::components::shortcuts::KeyboardShortcut;
use crate::components::song_picker;
use crate::components::song_picker::SongPicker;
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
    SelectSlide {
        item: usize,
        slide: usize,
    },
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
    /// Prezentační okno změnilo velikost
    PresentationWindowResized(Size),
    /// Byl otevřen výběr písní
    OpenSongPicker,
    /// Zpráva z výběru písní
    SongPicker(song_picker::Message),
    /// Byla vybrána nová položka, přidáme do playlistu
    AddToPlaylist(PlaylistItem),
    OpenBiblePicker,
    BiblePicker(bible_picker::Message),
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
            KeyboardShortcutMessage::OpenSongPicker => Message::OpenSongPicker,
            KeyboardShortcutMessage::OpenBiblePicker => Message::OpenBiblePicker,
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
    OpenSongPicker,
    OpenBiblePicker,
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::Presenter(value)
    }
}

#[derive(Debug)]
pub struct Presenter {
    /// Id okna s prezentací
    presentation_window: Option<PresentationWindow>,
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
    shortcuts: [KeyboardShortcut<KeyboardShortcutMessage>; 8],
    /// Rychlé vkládání pasáží/písní při prezentaci
    picker: OpenedPicker,
}

impl Presenter {
    /// Vytvoří nový `Presenter`. Playlist musí obsahovat alespoň jednu položku,
    /// jinak není co prezentovat a funkce vrátí Error.
    pub fn try_new(playlist: Playlist) -> Result<Presenter> {
        if playlist.items.is_empty() {
            Err(anyhow!("Nelze prezentovat prázdný playlist"))
        } else {
            Ok(Presenter {
                playlist,
                item_index: 0,
                item_slide_index: 0,
                mode: PresentationMode::Normal,
                presentation_window: None,
                text_scale: TEXT_SIZE_MULTIPLIER_DEFAULT_U8,
                picker: OpenedPicker::None,
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
                        Key::Character("p".into()),
                        Modifiers::empty(),
                        KeyboardShortcutMessage::OpenSongPicker,
                        "Přidat píseň",
                    ),
                    KeyboardShortcut::new(
                        Key::Character("b".into()),
                        Modifiers::SHIFT,
                        KeyboardShortcutMessage::OpenBiblePicker,
                        "Přidat verše",
                    ),
                    KeyboardShortcut::new(
                        Key::Character("f".into()),
                        Modifiers::empty(),
                        KeyboardShortcutMessage::FreezeOuput,
                        "Zmrazit výstup",
                    ),
                    KeyboardShortcut::new(
                        Key::Character("n".into()),
                        Modifiers::empty(),
                        KeyboardShortcutMessage::NormalOuput,
                        "Normální výstup",
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
        let window_resizes = window::resize_events()
            .with(self.presentation_window.map(|w| w.id))
            .filter_map(|(presentation_window_id, (id, size))| {
                if presentation_window_id.is_some_and(|w_id| w_id == id) {
                    Some(Message::PresentationWindowResized(size))
                } else {
                    None
                }
            })
            .map(crate::Message::from);

        let picker_specific = match &self.picker {
            OpenedPicker::Song(song_picker) => song_picker.subscription().map(Message::SongPicker),
            OpenedPicker::Passage(bible_picker) => {
                bible_picker.subscription().map(Message::BiblePicker)
            }
            OpenedPicker::None => {
                KeyboardShortcut::subscription(self.shortcuts.clone()).map(Message::from)
            }
        }
        .map(crate::Message::from);

        Subscription::batch([window_resizes, picker_specific])
    }

    pub fn get_presentation_window_id(&self) -> Option<Id> {
        self.presentation_window.map(|w| w.id)
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

    fn view_helper_slide_table(&self) -> table::Table<Message> {
        let slide_text_size = Pixels::from(16);
        let slide_text_lineheight = LineHeight::default().to_absolute(slide_text_size);

        let name_column = table::column("Název", |(i, playlist_item): (usize, &PlaylistItem)| {
            let selected = i == self.item_index;
            let style = playlist_item_styles::playlist_item_button_style2(playlist_item, selected);

            // Iced 1.14 má podivný layout v tabulkách, když dám do jednoho sloupce .height(Lentgh::Fill), tak se to nematchne na nejvyšší sloupec na daném řádku, ale spapá to všechno dostupné místo, musíme si tak pomoct a spočítat potřebnou výšku sami
            let (text, num_slides) = match playlist_item {
                PlaylistItem::BiblePassage(passage) => {
                    let translation = passage.get_translation_name();
                    let indices: VerseIndices = passage.get_range().into();
                    let text = text!("{indices} ({translation})");
                    let num_slides = passage.get_verses().iter().count();
                    (text, num_slides)
                }
                PlaylistItem::Song(song) => {
                    let text = text(song.title.clone()); // Tady asi nemusí být klonování nutné
                    let num_slides = song.order.len();
                    (text, num_slides)
                }
            };

            // Manuální výpočet výšky, aby se rovnala výšce buňky s náhledem na stejném řádku
            let button_height = Pixels::from(
                num_slides as f32
                    * (slide_text_lineheight.0
                        + button::DEFAULT_PADDING.top
                        + button::DEFAULT_PADDING.bottom),
            );

            button(text)
                .style(style)
                .height(button_height)
                .width(Length::Fill)
                .on_press(Message::SelectSlide { item: i, slide: 0 })
        });

        let preview_column = table::column(
            text("Náhled"),
            |(item_i, playlist_item): (usize, &PlaylistItem)| {
                // let style = playlist_item_styles::playlist_item_button_style2(playlist_item, selected);

                let item_selected = item_i == self.item_index;

                let content: Vec<String> = match playlist_item {
                    PlaylistItem::BiblePassage(passage) => passage
                        .get_verses()
                        .iter()
                        .map(|(index, content)| format!("{index}: {content}"))
                        .collect(),
                    PlaylistItem::Song(song) => song
                        .order
                        .iter()
                        .map(|key| {
                            let content = &song.parts[key].replace('\n', " ");
                            format!("[{key}]: {content}")
                        })
                        .collect(),
                };

                let slide_buttons = content
                    .into_iter()
                    .enumerate()
                    .map(|(slide_i, content)| {
                        let slide_selected = item_selected && slide_i == self.item_slide_index;
                        let style = playlist_item_button_style2(playlist_item, slide_selected);
                        button(
                            text(content)
                                .wrapping(text::Wrapping::None)
                                .line_height(slide_text_lineheight)
                                .size(slide_text_size),
                        )
                        .style(style)
                        .width(Length::Fill)
                        .on_press(Message::SelectSlide {
                            item: item_i,
                            slide: slide_i,
                        })
                    })
                    .map(Element::from);

                column(slide_buttons)
            },
        );

        table(
            [
                name_column.width(Length::FillPortion(1)),
                preview_column.width(Length::FillPortion(5)),
            ],
            self.playlist.items.iter().enumerate(),
        )
        .padding(0)
    }

    pub fn view_control(&self) -> Element<Message> {
        match &self.picker {
            OpenedPicker::Song(song_picker) => song_picker.view().map(Message::SongPicker),
            OpenedPicker::Passage(bible_picker) => bible_picker.view().map(Message::BiblePicker),
            OpenedPicker::None => self.view_control_no_picker(),
        }
    }

    /// Zkonstruuje GUI pro ovládací okno
    pub fn view_control_no_picker(&self) -> Element<Message> {
        let slide_list = self.view_helper_slide_table();

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
            button("Přidat píseň")
                .width(Length::Fill)
                .on_press(Message::OpenSongPicker),
            button("Přidat verše")
                .width(Length::Fill)
                .on_press(Message::OpenBiblePicker),
            space().height(Length::Fixed(30.0)),
            button("Ukončit prezentaci (ESC)")
                .width(Length::Fill)
                .style(danger)
                .on_press(Message::ClosePresentationWindow),
        ]
        .spacing(10)
        .padding(30);

        let preview = iter::from_fn(|| {
            self.presentation_window.map(|w| {
                column![
                    text("Náhled").align_x(Alignment::Center),
                    view_helper_preview(
                        &self.playlist.items,
                        self.item_index,
                        self.item_slide_index,
                        &self.mode,
                        w.size,
                        self.text_scale,
                    )
                ]
                .width(Length::Fill)
                .align_x(Alignment::Center)
                .height(Length::FillPortion(1))
                .into()
            })
        })
        .take(1);

        Into::<Element<Message>>::into(container(
            row![
                presentation_control
                    .width(Length::FillPortion(1))
                    .height(Length::Fill),
                column(
                    [scrollable(slide_list).height(Length::FillPortion(2)).into()]
                        .into_iter()
                        // [space().height(1000).width(Length::Fill).into()]
                        //     .into_iter()
                        .chain(preview)
                )
                .width(Length::FillPortion(2))
                .align_x(Alignment::Center)
                .spacing(10),
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
        // .explain(iced::Color::BLACK)
    }

    /// Vytvoří GUI pro prezentované okno
    pub fn view_presentation(&self) -> Element<Message> {
        view_presentation_helper(
            &self.playlist.items,
            self.item_index,
            self.item_slide_index,
            &self.mode,
            self.text_scale,
            1.0,
        )
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
                        .presentation_window
                        .expect("Nelze zavřít prezentační okno, pokud nebylo otevřeno")
                        .id,
                )
                .chain(Task::done(Message::PresentationWindowClosed.into()))
            }
            Message::PresentationWindowClosed => {
                state.screen = Screen::StartScreen(StartScreen::new());
                Task::none()
            }
            Message::OpenPresentationWindow => {
                debug!("Otevírám prezentační okno");
                let settings = Settings {
                    fullscreen: true,
                    ..Settings::default()
                };
                let size = settings.size;
                let (id, task) = iced::window::open(settings);
                presenter.presentation_window = Some(PresentationWindow::new(id, size));
                task.map(|id| Message::PresentationWindowOpened(id).into())
            }
            Message::PresentationWindowOpened(id) => {
                debug!("Prezentační okno otevřeno pod id {id}");
                assert!(
                    presenter.presentation_window.is_some_and(|w| w.id == id),
                    "Prezentační okno otevřeno pod jiným ID, jak je toto možné?"
                );
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
            Message::PresentationWindowResized(size) => {
                debug!(
                    "Prezentační okno změněno na velikost {}x{} (ŠxV)",
                    size.width, size.height
                );
                presenter
                    .presentation_window
                    .as_mut() // Velmi důležité, jinak to chceme referenci
                    .expect("Prezentační okno může změnit velikost pouze až potom co bylo otevřeno")
                    .size = size;
                Task::none()
            }
            Message::OpenSongPicker => {
                debug!("Otevírám výběr písně");
                presenter.picker = OpenedPicker::Song(SongPicker::new());
                Task::none()
            }
            Message::SongPicker(message) => {
                let picker = presenter
                    .picker
                    .as_song_mut()
                    .expect("Zpráva pro song_picker přišla, když song_picker nebyl vybrán");
                match message {
                    song_picker::Message::Return => {
                        presenter.picker = OpenedPicker::None;
                        Task::none()
                    }
                    song_picker::Message::ReturnSelected(picker_item) => {
                        debug!("Načítám píseň {:?}", &picker_item);

                        let conn = state.db.acquire();
                        Task::perform(
                            async move {
                                let mut conn =
                                    conn.await.context("Nelze získat připojení k databázi")?;
                                Song::load_from_db(picker_item.id, &mut conn).await
                            },
                            |res| match res {
                                Ok(s) => Message::AddToPlaylist(PlaylistItem::Song(s)).into(),
                                Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                            },
                        )
                    }
                    song_picker::Message::FatalError(e) => {
                        Task::done(crate::Message::FatalErrorOccured(e))
                    }
                    msg => picker
                        .update(&state.db, msg)
                        .map(|msg| Message::SongPicker(msg).into()),
                }
            }
            Message::AddToPlaylist(item) => {
                debug!("Přidávám do playlistu položku{:?}", item);
                presenter.playlist.push_item(item);
                presenter.picker = OpenedPicker::None;

                // Na pozadí uložíme playlist, protože se do něj přidala nová položka
                let metadata = presenter.playlist.metadata();
                let conn = state.db.acquire();
                let task = Task::future(async move {
                    let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                    metadata
                        .update(&mut conn)
                        .await
                        .inspect_err(|e| error!("Nelze uložit Playlist do databáze: {e}"))
                });

                task.discard()
            }
            Message::OpenBiblePicker => {
                debug!("Otevírám výběr pasáže");
                presenter.picker = OpenedPicker::Passage(BiblePicker::new());
                Task::none()
            }
            Message::BiblePicker(message) => {
                let picker = presenter
                    .picker
                    .as_passage_mut()
                    .expect("Zpráva pro bible_picker přišla, když bible_picker nebyl vybrán");
                match message {
                    bible_picker::Message::Return => {
                        presenter.picker = OpenedPicker::None;
                        Task::none()
                    }
                    bible_picker::Message::ReturnSelected(translation_id, from, to) => {
                        debug!("Načítám pasáž {from}-{to}");

                        let conn = state.db.acquire();
                        Task::perform(
                            async move {
                                let mut conn =
                                    conn.await.context("Nelze získat připojení k databázi")?;
                                Passage::load(from, to, translation_id, &mut conn).await
                            },
                            |res| match res {
                                Ok(p) => {
                                    Message::AddToPlaylist(PlaylistItem::BiblePassage(p)).into()
                                }
                                Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                            },
                        )
                    }
                    bible_picker::Message::FatalError(e) => {
                        Task::done(crate::Message::FatalErrorOccured(e))
                    }
                    msg => picker
                        .update(&state.db, msg)
                        .map(|msg| Message::BiblePicker(msg).into()),
                }
            }
        }
    }
}

/// Zkonstruuuje GUI prezentovaného slajdu
fn view_presentation_helper<'p>(
    items: &'p [PlaylistItem],
    item_index: usize,
    item_slide_index: usize,
    mode: &PresentationMode,
    text_scale: u8,
    scale: f32,
) -> Element<'p, Message> {
    let text_size_multiplier = normalize_text_multiplier(text_scale) * scale;
    let item = &items[item_index];

    match mode {
        PresentationMode::Normal => render_slide(item, item_slide_index, text_size_multiplier),
        PresentationMode::Blank => blank_slide(),
        PresentationMode::Frozen { item, item_slide } => {
            render_slide(&items[*item], *item_slide, text_size_multiplier)
        }
    }
}

fn view_helper_preview<'p>(
    items: &'p [PlaylistItem],
    item_index: usize,
    item_slide_index: usize,
    mode: &'p PresentationMode,
    presentation_window_size: Size,
    text_scale: u8,
) -> Element<'p, Message> {
    let content_build_closure = move |size: Size| {
        let scale1 = size.width / presentation_window_size.width;
        let scale2 = size.height / presentation_window_size.height;
        let scale = f32::min(scale1, scale2);

        let width = presentation_window_size.width * scale;
        let height = presentation_window_size.height * scale;

        let slide =
            view_presentation_helper(items, item_index, item_slide_index, mode, text_scale, scale);

        container(
            container(slide)
                .width(Length::Fixed(width))
                .height(Length::Fixed(height)),
        )
        .center_x(Length::Fill)
        .into()
    };

    responsive(content_build_closure).into()
}

/// Vytvoří grafickou reprezentaci `item_slide_index`-tého slajdu `item_index`-té položky.
/// Pokud je jeden z indexů neplatný, zpanikaří.
///
/// Text Bude Škálován pomocí `scale`.
fn render_slide(item: &PlaylistItem, item_slide_index: usize, scale: f32) -> Element<Message> {
    match item {
        PlaylistItem::BiblePassage(passage) => {
            let verses_text_size = MAIN_TEXT_SIZE * scale;
            let indexes_text_size = ADDITIONAL_TEXT_SIZE * scale;

            let verses_content: String = passage
                .get_verses()
                .chunks(VERSES_PER_SLIDE)
                .nth(item_slide_index)
                .expect("Index slajdu pasáže není validní")
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
        PlaylistItem::Song(song) => song.order.len(),
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct PresentationWindow {
    id: Id,
    size: Size,
}

impl PresentationWindow {
    fn new(id: Id, size: Size) -> Self {
        Self { id, size }
    }
}
