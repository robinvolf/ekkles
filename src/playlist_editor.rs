use std::sync::Arc;

use anyhow::{Context, Result};
use ekkles_data::{
    Song,
    bible::indexing::{Passage, VerseIndices},
    playlist::{self, PlaylistItem, PlaylistItemMetadata, PlaylistMetadata},
};
use iced::{
    Element, Length, Task,
    alignment::{Horizontal, Vertical},
    futures::{FutureExt, future::try_join_all},
    widget::{button, column, container, row, table, text, text_input},
};
use log::{debug, trace};
use sqlx::{Sqlite, pool::PoolConnection};
use tokio::sync::Mutex;

use crate::{
    Ekkles, Screen,
    bible_picker::BiblePicker,
    components::{LazyLoadable, LazyLoadableState, playlist_item_styles},
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
    StartPresentation(Presenter),
    AddBiblePassage,
    AddSong,
    SelectItem(usize),
    MoveItemUp(usize),
    MoveItemDown(usize),
    DeleteItem(usize),
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
    /// Jednotlivé položky playlistu pro náhled
    playlist_preview_items: LazyLoadable<Vec<PlaylistItem>, Message>,
    /// Název playlistu pro "uložit jako"
    new_playlist_name: String,
    /// Chybová hláška, v případě špatného nového názvu playlistu
    new_playlist_err_msg: String,
    /// Index právě vybrané položky. `Option`, protože je možné mít prázdný playlist.
    selected_index: Option<usize>,
}

impl PlaylistEditor {
    pub fn new(playlist: PlaylistMetadata) -> Self {
        Self {
            playlist,
            playlist_preview_items: LazyLoadable::new(Message::LoadPreview),
            new_playlist_name: String::new(),
            new_playlist_err_msg: String::new(),
            selected_index: None,
        }
    }

    pub async fn new_load(playlist_id: i64, conn: PoolConnection<Sqlite>) -> Result<Self> {
        let playlist = PlaylistMetadata::load(playlist_id, conn).await?;
        Ok(Self::new(playlist))
    }

    pub fn view(&self) -> Element<Message> {
        let playlist_name = self.playlist.name();

        // let playlist_items = self
        //     .playlist
        //     .items()
        //     .iter()
        //     .enumerate()
        //     .map(|(index, item)| {
        //         let msg = if self
        //             .selected_index
        //             .is_some_and(|selected| selected == index)
        //         {
        //             None
        //         } else {
        //             Some(Message::SelectItem(index))
        //         };

        //         let content = match item {
        //             playlist::PlaylistItemMetadata::BiblePassage { from, to, .. } => "Pasáž",
        //             playlist::PlaylistItemMetadata::Song(sought_id) => "Píseň",
        //         };

        //         button(content)
        //             .style(if msg.is_none() {
        //                 playlist_item_styles::song_selected
        //             } else {
        //                 playlist_item_styles::song
        //             })
        //             .on_press_maybe(msg)
        //             .width(Length::Fill)
        //             .into()
        //     });
        let playlist_items = table(
            [table::column(
                "Název",
                |(index, item): (usize, &PlaylistItemMetadata)| {
                    let msg = if self
                        .selected_index
                        .is_some_and(|selected| selected == index)
                    {
                        None
                    } else {
                        Some(Message::SelectItem(index))
                    };

                    let content: Element<Message> =
                        match (item, self.playlist_preview_items.state()) {
                            (_, LazyLoadableState::Cold | LazyLoadableState::Loading { .. }) => {
                                self.playlist_preview_items.view_not_loaded().into()
                            }
                            (
                                PlaylistItemMetadata::BiblePassage { .. },
                                LazyLoadableState::Loaded(preview_items),
                            ) => {
                                let preview_item = &preview_items[index];
                                let passage = preview_item.as_bible_passage().unwrap();
                                let indices = VerseIndices::from(passage.get_range());
                                text!("{} {}", passage.get_translation_name(), indices).into()
                            }
                            (
                                PlaylistItemMetadata::Song(_),
                                LazyLoadableState::Loaded(preview_items),
                            ) => {
                                let preview_item = &preview_items[index];
                                let song = preview_item.as_song().unwrap();
                                text!("Píseň {}", song.title).into()
                            }
                        };

                    button(content)
                        .style(if msg.is_none() {
                            playlist_item_styles::song_selected
                        } else {
                            playlist_item_styles::song
                        })
                        .on_press_maybe(msg)
                        .width(Length::Fill)
                },
            )],
            self.playlist.items().iter().enumerate(),
        )
        .padding(30);

        let item_manipulation = match self.selected_index {
            Some(index) => {
                column![
                    button("Posunout nahoru")
                        .on_press_maybe(if index == 0 {
                            None
                        } else {
                            Some(Message::MoveItemUp(index))
                        })
                        .width(Length::Fill),
                    button("Posunout dolů")
                        // len() - 1 je v pořádku, nikdy nepodteče, tento kód se provede pouze
                        // s vybranou položkou, nelze mít vybranou položku na prázdném seznamu
                        .on_press_maybe(if index == self.playlist.items().len() - 1 {
                            None
                        } else {
                            Some(Message::MoveItemDown(index))
                        })
                        .width(Length::Fill),
                    button("Smazat položku")
                        .on_press(Message::DeleteItem(index))
                        .style(button::danger)
                        .width(Length::Fill),
                ]
            }
            None => column([]),
        };

        Into::<Element<Message>>::into(column![
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
                            .on_press(Message::AddSong)
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
                playlist_items.width(Length::FillPortion(2)),
                if self.selected_index.is_some() {
                    item_manipulation
                } else {
                    column([])
                }
                .width(Length::FillPortion(1))
                .padding(30)
                .spacing(10),
            ])
            .padding(10)
            .center_x(Length::FillPortion(1))
        ])
        // .explain(Color::BLACK)
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

                        Presenter::try_new(*playlist.id(), &mut conn).await
                    },
                    |res| match res {
                        Ok(presenter) => Message::StartPresentation(presenter).into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::StartPresentation(presenter) => {
                debug!("Přecházím na prezentační obrazovku");
                state.screen = Screen::Presenter(presenter);
                Task::done(crate::presenter::Message::OpenPresentationWindow.into())
            }
            Message::AddBiblePassage => {
                debug!("Přecházím na výběr playlistu");
                // let playlist = editor.playlist.blocking_lock().clone();
                // state.screen = Screen::PickBible(BiblePicker::new(playlist));
                // Task::done(crate::Message::BiblePicker(
                //     crate::bible_picker::Message::LoadTranslations,
                // ))
                todo!()
            }
            Message::AddSong => {
                debug!("Přecházím na výběr písně");
                // let playlist = editor.playlist.blocking_lock().clone();
                todo!();
                // state.screen = Screen::PickSong(SongPicker::new(playlist));
                // Task::done(crate::Message::SongPicker(
                //     crate::song_picker::Message::LoadSongs,
                // ))
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
                state.screen = Screen::PickEditor(StartScreen::default());
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
            Message::MoveItemUp(index) => {
                debug!("Posunuji položku na indexu {index} na {}", index - 1);
                *editor
                    .selected_index
                    .as_mut()
                    .expect("Při posunování vybrané položka musí být položka vybrána") -= 1;
                editor
                    .playlist
                    .swap_items(index, index - 1)
                    .expect("Nelze posunout položku nahoru");
                Task::none()
            }
            Message::MoveItemDown(index) => {
                debug!("Posunuji položku na indexu {index} na {}", index + 1);
                *editor
                    .selected_index
                    .as_mut()
                    .expect("Při posunování vybrané položka musí být položka vybrána") += 1;

                editor
                    .playlist
                    .swap_items(index, index + 1)
                    .expect("Nelze posunout položku dolů");
                Task::none()
            }
            Message::DeleteItem(index) => {
                debug!("Mažu položku s indexem {index}");
                editor.selected_index = None;
                editor
                    .playlist
                    .delete_item(index)
                    .expect("Nelze smazat položku");
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
        }
    }
}
