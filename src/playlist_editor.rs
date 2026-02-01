//! # Fičurky editoru
//!
//! ## Kurzor
//! - Téměř vždy (vyjma prázdného playlistu) existuje kurzor - aktuálně vybraná položka playlistu
//! - Poloha kurzoru se mění:
//!   - Kliknutím na položku
//!   - Klávesových zkratek
//!
//! ## Posunování položek
//! - Položka pod kurzorem je posunovatelná pomocí:
//!   - Tlačítek v pravém panelu nahoru/dolů
//!   - Klávesových zkratek
//!
//! ## Mazání položek
//! - Stejně jako posouvání tlačítko/klávesová zkratka
//! - Kurzor bude po smazání položky nastaven na:
//!   + Pokud ještě zbývají nějaké _následující_ položky, kurzor se nastaví na následující položku
//!   + Pokud ještě zbývají nějaké _předchozí_ položky, kurzor se nastaví na předchozí položku
//!   + Pokud je po smazání položky playlist prázdný, kurzor zmizí
//!
//! ## Přidávání položek
//! - Položky se přidávají pomocí tlačítek (nová pasáž/nová píseň) nebo klávesových zkratek (TODO)
//! - Položka se vždy přidá _za_ položku označenou kurzorem

use anyhow::{Context, Result};
use ekkles_data::{
    bible::indexing::VerseIndices,
    playlist::{self, Playlist, PlaylistItem, PlaylistItemMetadata, PlaylistMetadata},
};
use iced::{
    Element, Length, Subscription, Task,
    alignment::{Horizontal, Vertical},
    keyboard::{Key, Modifiers, key},
    widget::{button, column, container, row, scrollable, space, table, text, text_input},
};
use log::{debug, trace};
use sqlx::{Sqlite, pool::PoolConnection};

use crate::{
    Ekkles, Screen,
    components::{
        LazyLoadable, LazyLoadableState, OpenedPicker,
        bible_picker::{self, BiblePicker},
        playlist_item_styles::playlist_item_button_style,
        shortcuts::KeyboardShortcut,
        song_picker::{self, SongPicker},
    },
    presenter::Presenter,
    start_screen::StartScreen,
};

#[derive(Debug, Clone)]
pub enum Message {
    SavePlaylist,
    PlaylistSavedSuccessfully,
    SavePlaylistAsClicked,
    NewPlaylistNameChanged(String),
    ValidateNewPlaylistName,
    InvalidNewPlaylistName(String),
    LoadPreview,
    PreviewLoaded {
        preview: Vec<PlaylistItem>,
        task_id: u32,
    },
    SavePlaylistAs,
    DeletePlaylist,
    SaveAndExit,
    ReturnToPlaylistPicker,
    LoadPresentation,
    StartPresentation(Playlist),
    AddBiblePassage,
    OpenSongPicker,
    SelectItem(usize),
    MoveSelectionUp,
    MoveSelectionDown,
    MoveItemUp,
    MoveItemDown,
    DeleteItem,
    SongPicker(song_picker::Message),
    BiblePicker(bible_picker::Message),
}

#[derive(Clone, Copy, Debug, Hash)]
enum KeyboardShortcutMessage {
    MoveItemUp,
    MoveItemDown,
    MoveSelectionUp,
    MoveSelectionDown,
    DeleteItem,
    ReturnToPlaylistPicker,
    OpenBiblePicker,
    OpenSongPicker,
}

impl From<KeyboardShortcutMessage> for Message {
    fn from(value: KeyboardShortcutMessage) -> Self {
        match value {
            KeyboardShortcutMessage::MoveItemUp => Message::MoveItemUp,
            KeyboardShortcutMessage::MoveItemDown => Message::MoveItemDown,
            KeyboardShortcutMessage::MoveSelectionUp => Message::MoveSelectionUp,
            KeyboardShortcutMessage::MoveSelectionDown => Message::MoveSelectionDown,
            KeyboardShortcutMessage::DeleteItem => Message::DeleteItem,
            KeyboardShortcutMessage::ReturnToPlaylistPicker => Message::ReturnToPlaylistPicker,
            KeyboardShortcutMessage::OpenBiblePicker => Message::AddBiblePassage,
            KeyboardShortcutMessage::OpenSongPicker => Message::OpenSongPicker,
        }
    }
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::PlaylistEditor(value)
    }
}

#[derive(Debug)]
pub struct PlaylistEditor {
    /// Aktuálně upravovaný playlist
    playlist: PlaylistMetadata,
    /// Jednotivé položky playlistu pro náhled
    playlist_preview_items: LazyLoadable<Vec<PlaylistItem>, Message>,
    /// Název playlistu pro "uložit jako"
    new_playlist_name: String,
    /// Chybová hláška, v případě špatného nového názvu playlistu
    new_playlist_err_msg: String,
    /// Index právě vybrané položky. `Option`, protože je možné mít prázdný playlist.
    selected_index: Option<usize>,
    /// Aktuální výběr, překresluje aktuální okno
    picker: OpenedPicker,
    shortcuts: [KeyboardShortcut<KeyboardShortcutMessage>; 8],
}

impl PlaylistEditor {
    pub fn new(playlist: PlaylistMetadata) -> Self {
        Self {
            playlist,
            playlist_preview_items: LazyLoadable::new(Message::LoadPreview),
            new_playlist_name: String::new(),
            new_playlist_err_msg: String::new(),
            selected_index: None,
            picker: OpenedPicker::None,
            shortcuts: [
                KeyboardShortcut::new(
                    Key::Named(key::Named::ArrowUp),
                    Modifiers::SHIFT,
                    KeyboardShortcutMessage::MoveItemUp,
                    "Posunout položku nahoru",
                ),
                KeyboardShortcut::new(
                    Key::Named(key::Named::ArrowDown),
                    Modifiers::SHIFT,
                    KeyboardShortcutMessage::MoveItemDown,
                    "Posunout položku dolů",
                ),
                KeyboardShortcut::new(
                    Key::Named(key::Named::ArrowUp),
                    Modifiers::empty(),
                    KeyboardShortcutMessage::MoveSelectionUp,
                    "Posunout kurzor nahoru",
                ),
                KeyboardShortcut::new(
                    Key::Named(key::Named::ArrowDown),
                    Modifiers::empty(),
                    KeyboardShortcutMessage::MoveSelectionDown,
                    "Posunout kurzor dolů",
                ),
                KeyboardShortcut::new(
                    Key::Character("d".into()),
                    Modifiers::empty(),
                    KeyboardShortcutMessage::DeleteItem,
                    "Smazat položku",
                ),
                KeyboardShortcut::new(
                    Key::Named(key::Named::Escape),
                    Modifiers::empty(),
                    KeyboardShortcutMessage::ReturnToPlaylistPicker,
                    "Zpět na výběr playlistů",
                ),
                KeyboardShortcut::new(
                    Key::Character("s".into()),
                    Modifiers::empty(),
                    KeyboardShortcutMessage::OpenSongPicker,
                    "Přidat Píseň",
                ),
                KeyboardShortcut::new(
                    Key::Character("b".into()),
                    Modifiers::empty(),
                    KeyboardShortcutMessage::OpenBiblePicker,
                    "Přidat verše",
                ),
            ],
        }
    }

    pub async fn new_load(playlist_id: i64, conn: PoolConnection<Sqlite>) -> Result<Self> {
        let playlist = PlaylistMetadata::load(playlist_id, conn).await?;
        Ok(Self::new(playlist))
    }

    pub fn view(&self) -> Element<Message> {
        match &self.picker {
            OpenedPicker::Song(song_picker) => song_picker.view().map(Message::SongPicker),
            OpenedPicker::Passage(bible_picker) => bible_picker.view().map(Message::BiblePicker),
            OpenedPicker::None => self.view_editor(),
        }
    }

    pub fn view_editor(&self) -> Element<Message> {
        let playlist_name = self.playlist.name();

        let playlist_items = container(playlist_table(self));

        let item_manipulation = match self.selected_index {
            Some(index) => {
                column![
                    button("Posunout nahoru")
                        .on_press_maybe(if index == 0 {
                            None
                        } else {
                            Some(Message::MoveItemUp)
                        })
                        .width(Length::Fill),
                    button("Posunout dolů")
                        // len() - 1 je v pořádku, nikdy nepodteče, tento kód se provede pouze
                        // s vybranou položkou, nelze mít vybranou položku na prázdném seznamu
                        .on_press_maybe(if index == self.playlist.items().len() - 1 {
                            None
                        } else {
                            Some(Message::MoveItemDown)
                        })
                        .width(Length::Fill),
                    button("Smazat položku")
                        .on_press(Message::DeleteItem)
                        .style(button::danger)
                        .width(Length::Fill),
                ]
            }
            None => column([]),
        };

        let right_column = column![
            if self.selected_index.is_some() {
                item_manipulation
            } else {
                column([])
            }
            .padding(30)
            .spacing(10)
            .height(Length::Fill),
            container(KeyboardShortcut::view(&self.shortcuts))
                .align_bottom(Length::Fill)
                .padding(30)
        ];

        Into::<Element<Message>>::into(
            container(row![
                column![
                    column![
                        text(format!("Edituješ playlist \"{}\"", playlist_name)),
                        button("Uložit")
                            .on_press(Message::SavePlaylist)
                            .width(Length::Fill),
                        row![
                            text_input("Název nového playlistu", &self.new_playlist_name)
                                .on_input(Message::NewPlaylistNameChanged)
                                .on_submit(Message::SavePlaylistAsClicked),
                            button("Uložit jako").on_press(Message::SavePlaylistAsClicked)
                        ]
                        .width(Length::Fill),
                        text(&self.new_playlist_err_msg)
                            .style(text::danger)
                            .width(Length::Fill),
                        button("Smazat playlist")
                            .style(button::danger)
                            .on_press(Message::DeletePlaylist)
                            .width(Length::Fill),
                        button("Přidat píseň")
                            .on_press(Message::OpenSongPicker)
                            .width(Length::Fill),
                        button("Přidat verše")
                            .on_press(Message::AddBiblePassage)
                            .width(Length::Fill),
                        button("Prezentovat")
                            .on_press(Message::LoadPresentation)
                            .width(Length::Fill)
                    ]
                    .width(Length::Fill)
                    .padding(30)
                    .spacing(10),
                    container(
                        button("Zpět")
                            .width(Length::Fill)
                            .on_press(Message::SaveAndExit)
                    )
                    .padding(30)
                    .align_y(Vertical::Bottom)
                    .height(Length::Fill)
                    .width(Length::Fill)
                ]
                .width(Length::FillPortion(1))
                .align_x(Horizontal::Center),
                playlist_items.width(Length::FillPortion(3)),
                right_column.width(Length::FillPortion(1)),
            ])
            .padding(10)
            .center_x(Length::FillPortion(1)),
        )
    }

    pub fn view_bible_picker(picker: &BiblePicker) -> Element<Message> {
        Into::<Element<Message>>::into(column![
            container(row![
                space().width(Length::FillPortion(1)),
                container(picker.view().map(|msg| Message::BiblePicker(msg)))
                    .width(Length::FillPortion(3))
                    .max_width(1000),
                space().width(Length::FillPortion(1)),
            ])
            .padding(10)
            .center_x(Length::FillPortion(1))
        ])
    }

    /// Update funkce pro editor. Pokud je tato funkce zavolána nad jinou obrazovkou
    /// než [`Screen::EditPlaylist`], zpanikaří.
    pub fn update(state: &mut Ekkles, msg: Message) -> Task<crate::Message> {
        let editor = match &mut state.screen {
            Screen::EditPlaylist(editor) => editor,
            screen => panic!("Update pro Editor zavolán, nad obrazovkou {:#?}", screen),
        };

        match msg {
            Message::SavePlaylist => {
                debug!("Ukládám playlist");
                let conn = state.db.acquire();
                let playlist = editor.playlist.clone();
                Task::perform(
                    async move {
                        let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                        playlist.update(&mut conn).await
                    },
                    |res| match res {
                        Ok(_) => Message::PlaylistSavedSuccessfully.into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::SavePlaylistAs => {
                debug!(
                    "Ukládám playlist pod novým názvem: \"{}\"",
                    &editor.new_playlist_name
                );
                let conn = state.db.acquire();
                let new_playlist_name = editor.new_playlist_name.clone();
                let new_playlist_items = editor.playlist.items().to_vec();

                Task::perform(
                    async move {
                        let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                        PlaylistMetadata::create_with_items(
                            &new_playlist_name,
                            &new_playlist_items,
                            &mut conn,
                        )
                        .await
                    },
                    |res| match res {
                        Ok(_) => Message::PlaylistSavedSuccessfully.into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::LoadPresentation => {
                debug!("Načítám prezentaci");
                let conn = state.db.acquire();
                let playlist = editor.playlist.clone();
                Task::perform(
                    async move {
                        let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                        playlist
                            .update(&mut conn)
                            .await
                            .context("Nelze uložit playlist")?;

                        Playlist::load(*playlist.id(), &mut conn).await
                    },
                    |res| match res {
                        Ok(playlist) => Message::StartPresentation(playlist).into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::StartPresentation(playlist) => {
                let presenter = Presenter::try_new(playlist);
                match presenter {
                    Ok(p) => {
                        debug!("Přecházím na prezentační obrazovku");
                        state.screen = Screen::Presenter(p);
                        Task::done(crate::presenter::Message::OpenPresentationWindow.into())
                    }
                    Err(e) => Task::done(crate::Message::FatalErrorOccured(format!("{:?}", e))),
                }
            }
            Message::AddBiblePassage => {
                debug!("Přecházím na výběr playlistu");
                editor.picker = OpenedPicker::Passage(BiblePicker::new());
                Task::none()
            }
            Message::OpenSongPicker => {
                debug!("Přecházím na výběr písně");
                editor.picker = OpenedPicker::Song(SongPicker::new());
                Task::none()
            }
            Message::PlaylistSavedSuccessfully => {
                debug!("Playlist byl úspéšně uložen");
                editor.new_playlist_name.clear();
                editor.new_playlist_err_msg.clear();
                Task::none()
            }
            Message::SavePlaylistAsClicked => Task::done(Message::ValidateNewPlaylistName.into()),
            Message::ValidateNewPlaylistName => {
                debug!("Validuji nové jméno pro playlist");
                if editor.new_playlist_name.trim().is_empty() {
                    return Task::done(
                        Message::InvalidNewPlaylistName(String::from("Prázdné jméno není validní"))
                            .into(),
                    );
                }

                debug!("Zjišťuji, jestli se v databázi nachází playlist s daným názvem");
                let conn = state.db.acquire();
                let name = editor.new_playlist_name.clone();
                Task::perform(
                    async move {
                        let conn = conn.await.context("Nelze získat připojení k databázi")?;
                        playlist::is_name_available(conn, &name).await
                    },
                    |res| match res {
                        Ok(available) => {
                            if available {
                                Message::SavePlaylistAs.into()
                            } else {
                                Message::InvalidNewPlaylistName(String::from(
                                    "Takové jméno se již nachází v databázi, vyber jiné",
                                ))
                                .into()
                            }
                        }
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::InvalidNewPlaylistName(err_msg) => {
                debug!("Nastavuji chybovou hlášku, aby uživatel změnil název nového playlistu");
                editor.new_playlist_err_msg = err_msg;
                Task::none()
            }
            Message::NewPlaylistNameChanged(input) => {
                trace!("Změnil se nový název playlistu: {input}");
                editor.new_playlist_name = input;
                Task::none()
            }
            Message::DeletePlaylist => {
                {
                    debug!("Mažu playlist \"{}\"", editor.playlist.name())
                }

                let conn = state.db.acquire();
                let id = *editor.playlist.id();
                Task::perform(
                    async move {
                        let mut conn = conn.await?;
                        PlaylistMetadata::delete_by_id(id, &mut conn).await
                    },
                    |res| match res {
                        Ok(_) => Message::ReturnToPlaylistPicker.into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::ReturnToPlaylistPicker => {
                state.screen = Screen::StartScreen(StartScreen::default());
                Task::none()
            }
            Message::SaveAndExit => {
                debug!("Ukládám playlist a vracím se k výběru playlistů");
                let conn = state.db.acquire();
                let playlist = editor.playlist.clone();
                Task::perform(
                    async move {
                        let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                        playlist.update(&mut conn).await
                    },
                    |res| res,
                )
                .then(|res| {
                    debug!("Playlist uložen, vracím se na výběr playlistů");
                    match res {
                        Ok(_) => Task::done(Message::ReturnToPlaylistPicker.into()),
                        Err(e) => Task::done(crate::Message::FatalErrorOccured(format!("{:?}", e))),
                    }
                })
            }
            Message::SelectItem(index) => {
                debug!("Vybrána položka playlistu {index}");
                editor.selected_index = Some(index);
                Task::none()
            }
            Message::MoveItemUp => {
                match editor.selected_index {
                    Some(index) if index > 0 => {
                        debug!("Posunuji položku na indexu {index} na {}", index - 1);
                        *editor
                            .selected_index
                            .as_mut()
                            .expect("Při posunování vybrané položka musí být položka vybrána") -= 1;
                        editor
                            .playlist
                            .swap_items(index, index - 1)
                            .expect("Nelze posunout položku nahoru");

                        if let LazyLoadableState::Loaded(preview) =
                            editor.playlist_preview_items.state_mut()
                        {
                            preview.swap(index, index - 1);
                        }
                    }
                    Some(_) => {
                        debug!(
                            "Pokus o posunutí položky nahoru, ale již je první v seznamu, ignoruju"
                        )
                    }
                    None => debug!("Pokus o posunutí položky nahoru bez kurzoru, ignoruju"),
                };
                Task::none()
            }
            Message::MoveItemDown => {
                match editor.selected_index {
                    Some(index) if index < editor.playlist.items().len() - 1 => {
                        debug!("Posunuji položku na indexu {index} na {}", index + 1);
                        *editor
                            .selected_index
                            .as_mut()
                            .expect("Při posunování vybrané položka musí být položka vybrána") += 1;
                        editor
                            .playlist
                            .swap_items(index, index + 1)
                            .expect("Nelze posunout položku nahoru");

                        if let LazyLoadableState::Loaded(preview) =
                            editor.playlist_preview_items.state_mut()
                        {
                            preview.swap(index, index + 1);
                        }
                    }
                    Some(_) => debug!(
                        "Pokus o posunutí položky dolů, ale již je poslední položka, ignoruju"
                    ),
                    None => debug!("Pokus o posunutí položky dolů bez kurzoru, ignoruju"),
                };
                Task::none()
            }
            Message::DeleteItem => {
                match editor.selected_index {
                    Some(index) => {
                        debug!("Mažu položku s indexem {index}");
                        editor
                            .playlist
                            .delete_item(index)
                            .expect("Nelze smazat položku");

                        if let LazyLoadableState::Loaded(preview) =
                            editor.playlist_preview_items.state_mut()
                        {
                            preview.remove(index);
                        }
                    }
                    None => debug!("Pokus o smazání položky dolů bez kurzoru, ignoruju"),
                }
                Task::none()
            }
            Message::LoadPreview => {
                let conn = state.db.acquire();
                let items = editor.playlist.items().to_vec();
                let (task, handle) = Task::abortable(Task::future(async move {
                    let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                    let items = items.iter();
                    PlaylistItem::load_list(items, &mut conn).await
                }));
                match editor.playlist_preview_items.start_loading(handle) {
                    Some(task_id) => {
                        let task = Task::map(task, move |res| match res {
                            Ok(preview) => Message::PreviewLoaded { preview, task_id }.into(),
                            Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                        });
                        task
                    }
                    None => Task::none(),
                }
            }
            Message::PreviewLoaded { preview, task_id } => {
                editor
                    .playlist_preview_items
                    .finish_loading(preview, task_id);
                Task::none()
            }
            Message::MoveSelectionUp => {
                match editor.selected_index {
                    Some(index)
                        if index > 0 && editor.playlist.items().get(index - 1).is_some() =>
                    {
                        editor.selected_index = Some(index - 1)
                    }
                    Some(_) => trace!("Nelze posunout kurzor nad první položku"),
                    None if !editor.playlist.items().is_empty() => {
                        editor.selected_index = Some(editor.playlist.items().len() - 1)
                    }
                    None => trace!("Nelze posunout kurzor na prázdném playlistu"),
                }
                Task::none()
            }
            Message::MoveSelectionDown => {
                match editor.selected_index {
                    Some(index) if editor.playlist.items().get(index + 1).is_some() => {
                        editor.selected_index = Some(index + 1)
                    }
                    Some(_) => trace!("Nelze posunout kurzor pod poslední položku"),
                    None if !editor.playlist.items().is_empty() => editor.selected_index = Some(0),
                    None => trace!("Nelze posunout kurzor na prázdném playlistu"),
                }
                Task::none()
            }
            Message::SongPicker(message) => {
                let picker = editor
                    .picker
                    .as_song_mut()
                    .expect("Zpráva pro song_picker přišla, když song_picker nebyl vybrán");
                match message {
                    song_picker::Message::Return => {
                        editor.picker = OpenedPicker::None;
                        Task::none()
                    }
                    song_picker::Message::ReturnSelected(picker_item) => {
                        debug!("Přidávám píseň {:?}", &picker_item);

                        // Přidání vybrané písně do playlistu
                        match editor.selected_index {
                            Some(index) => editor.playlist.add_song(picker_item.id, index),
                            None => editor.playlist.push_song(picker_item.id),
                        }

                        // Musíme znovu načíst cache pro náhled
                        editor.playlist_preview_items.invalidate();

                        // Zavřeme výběr písně
                        editor.picker = OpenedPicker::None;

                        Task::none()
                    }
                    song_picker::Message::FatalError(e) => {
                        Task::done(crate::Message::FatalErrorOccured(e))
                    }
                    msg => picker
                        .update(&state.db, msg)
                        .map(|msg| Message::SongPicker(msg).into()),
                }
            }
            Message::BiblePicker(message) => {
                let picker = editor
                    .picker
                    .as_passage_mut()
                    .expect("Zpráva pro song_picker přišla, když song_picker nebyl vybrán");
                match message {
                    bible_picker::Message::Return => {
                        editor.picker = OpenedPicker::None;
                        Task::none()
                    }
                    bible_picker::Message::ReturnSelected(translation_id, from, to) => {
                        debug!("Přidávám pasáž {from} - {to} (id překladu: {translation_id})");

                        // Přidání vybrané písně do playlistu
                        match editor.selected_index {
                            Some(index) => {
                                editor
                                    .playlist
                                    .add_bible_passage(translation_id, from, to, index)
                            }
                            None => editor.playlist.push_bible_passage(translation_id, from, to),
                        }

                        // Musíme znovu načíst cache pro náhled
                        editor.playlist_preview_items.invalidate();

                        // Zavřeme výběr písně
                        editor.picker = OpenedPicker::None;

                        Task::none()
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

    pub fn subscription(&self) -> Subscription<crate::Message> {
        match &self.picker {
            OpenedPicker::Song(song_picker) => song_picker
                .subscription()
                .map(Message::SongPicker)
                .map(crate::Message::from),
            OpenedPicker::Passage(bible_picker) => bible_picker
                .subscription()
                .map(Message::BiblePicker)
                .map(crate::Message::from),
            OpenedPicker::None => KeyboardShortcut::subscription(self.shortcuts.clone())
                .map(Message::from)
                .map(crate::Message::from),
        }
    }
}

fn playlist_table(editor: &PlaylistEditor) -> Element<Message> {
    let playlist_item_content_column = |(index, item): (usize, &PlaylistItemMetadata)| {
        let is_selected = editor
            .selected_index
            .is_some_and(|selected| selected == index);
        let msg = if is_selected {
            None
        } else {
            Some(Message::SelectItem(index))
        };

        let content: Element<Message> = match (item, editor.playlist_preview_items.state()) {
            (_, LazyLoadableState::Cold | LazyLoadableState::Loading { .. }) => {
                editor.playlist_preview_items.view_not_loaded().into()
            }
            (
                PlaylistItemMetadata::BiblePassage { .. },
                LazyLoadableState::Loaded(preview_items),
            ) => {
                let preview_item = &preview_items[index];
                let passage = preview_item.as_bible_passage().unwrap();
                let indices = VerseIndices::from(passage.get_range());
                text!("{} ({})", indices, passage.get_translation_name())
                    .wrapping(text::Wrapping::None)
                    .into()
            }
            (PlaylistItemMetadata::Song(_), LazyLoadableState::Loaded(preview_items)) => {
                let preview_item = &preview_items[index];
                let song = preview_item.as_song().unwrap();
                text(&song.title).wrapping(text::Wrapping::None).into()
            }
        };

        button(content)
            .style(playlist_item_button_style(item, is_selected))
            .on_press_maybe(msg)
            .width(Length::Fill)
            .clip(true)
    };

    let playlist_item_kind_column = |(index, &item)| {
        let is_selected = editor
            .selected_index
            .is_some_and(|selected| selected == index);
        let msg = if is_selected {
            None
        } else {
            Some(Message::SelectItem(index))
        };

        let text = match item {
            PlaylistItemMetadata::BiblePassage { .. } => "Pasáž",
            PlaylistItemMetadata::Song(_) => "Píseň",
        };

        button(text)
            .on_press_maybe(msg)
            .style(playlist_item_button_style(&item, is_selected))
            .height(Length::Shrink)
            .width(Length::Fill)
    };

    let playlist_item_preview_column = |(index, &item)| {
        let is_selected = editor
            .selected_index
            .is_some_and(|selected| selected == index);

        let msg = if is_selected {
            None
        } else {
            Some(Message::SelectItem(index))
        };

        let content: Element<Message> = match editor.playlist_preview_items.state() {
            LazyLoadableState::Cold | LazyLoadableState::Loading { .. } => {
                editor.playlist_preview_items.view_not_loaded().into()
            }
            LazyLoadableState::Loaded(preview) => {
                let preview_item = &preview[index];
                const MAX_PREVIEW_VERSES: usize = 3;
                let content = match preview_item {
                    PlaylistItem::BiblePassage(passage) => passage
                        .get_verses()
                        .iter()
                        .take(MAX_PREVIEW_VERSES)
                        .map(|(num, text)| format!("{num}: {text} "))
                        .chain(["...".into()])
                        .collect::<String>(),

                    PlaylistItem::Song(song) => song
                        .order
                        .iter()
                        .map(|key| {
                            let removed_newlines = song.parts[key].replace('\n', " ");
                            format!("{}: {} ", key, removed_newlines)
                        })
                        .chain(["...".into()])
                        .collect(),
                };

                text(content).wrapping(text::Wrapping::None).into()
            }
        };

        button(content)
            .clip(true)
            .on_press_maybe(msg)
            .style(playlist_item_button_style(&item, is_selected))
            .height(Length::Shrink)
            .width(Length::Fill)
    };

    container(scrollable(
        table(
            [
                table::column("Druh", playlist_item_kind_column).width(Length::Fixed(70.0)), // Obsah tohoto sloupce je "Píseň"/"Pasáž", tudíž nastavíme absolutní šířku
                table::column("Název", playlist_item_content_column).width(Length::FillPortion(1)),
                table::column("Náhled", playlist_item_preview_column).width(Length::FillPortion(2)),
            ],
            editor.playlist.items().iter().enumerate(),
        )
        .separator(1)
        .padding(0),
    ))
    .into()
}
