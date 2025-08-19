use std::sync::Arc;

use anyhow::Context;
use ekkles_data::playlist::{self, PlaylistMetadata, PlaylistMetadataStatus};
use iced::{
    Color, Element, Length, Task,
    alignment::{Horizontal, Vertical},
    widget::{button, column, container, row, text, text_input},
};
use log::{debug, trace};
use tokio::sync::Mutex;

use crate::{
    Ekkles, Screen,
    components::{TopButtonsMessage, TopButtonsPickedSection, top_buttons},
    pick_playlist::PlaylistPicker,
};

#[derive(Debug, Clone)]
pub enum Message {
    TopButtonsPlaylist,
    TopButtonsSongs,
    SavePlaylist,
    PlaylistSavedSuccessfully,
    SavePlaylistAsClicked,
    NewPlaylistNameChanged(String),
    ValidateNewPlaylistName,
    NewPlaylistNameTaken,
    SavePlaylistAs,
    DeletePlaylist,
    SaveAndExit,
    ReturnToPlaylistPicker,
    StartPresentation,
    AddBiblePassage,
    AddSong,
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::PlaylistEditor(value)
    }
}

impl From<TopButtonsMessage> for Message {
    fn from(value: TopButtonsMessage) -> Self {
        match value {
            TopButtonsMessage::Playlists => Message::TopButtonsPlaylist,
            TopButtonsMessage::Songs => Message::TopButtonsSongs,
        }
    }
}

#[derive(Debug)]
pub struct PlaylistEditor {
    /// Editovaný playlist (potřebujeme ho zabalit do `Arc<Mutex<>>`, protože když jej ukládáme,
    /// mutujeme jeho stav a protože daný future předáme iced runtime, nemůže to být reference).
    playlist: Arc<Mutex<PlaylistMetadata>>,
    is_saved: bool,
    new_playlist_name: String,
    new_playlist_err_msg: String,
    song_name_cache: Option<Vec<(i64, String)>>,
}

impl PlaylistEditor {
    pub fn new(playlist: PlaylistMetadata) -> Self {
        Self {
            is_saved: match playlist.get_status() {
                playlist::PlaylistMetadataStatus::Transient => false,
                playlist::PlaylistMetadataStatus::Clean(_) => true,
                playlist::PlaylistMetadataStatus::Dirty(_) => panic!(
                    "Právě jsme dostali dirty playlist do editoru, to by se nikdy nemělo stát"
                ),
            },
            playlist: Arc::new(Mutex::new(playlist)),
            new_playlist_name: String::new(),
            new_playlist_err_msg: String::new(),
            song_name_cache: None,
        }
    }

    pub fn view(&self) -> Element<Message> {
        let (playlist_status, playlist_name) = {
            // Tady blokuju čekáním na mutex v GUI kódu, ale contention tohoto mutexu
            // je prakticky nulová (zamykám ho jen při zápisu do DB, který je velice rychlý).
            let playlist = self.playlist.blocking_lock();
            let status = playlist.get_status();
            let name = playlist.get_name().to_string();
            (status, name)
        }; // V separátním scope, abychom tady dropli mutex

        let (save_button_msg, delete_button_msg) = match playlist_status {
            playlist::PlaylistMetadataStatus::Transient => (Some(Message::SavePlaylist), None),
            playlist::PlaylistMetadataStatus::Clean(_) => (None, Some(Message::DeletePlaylist)),
            playlist::PlaylistMetadataStatus::Dirty(_) => {
                (Some(Message::SavePlaylist), Some(Message::DeletePlaylist))
            }
        };

        Into::<Element<Message>>::into(column![
            top_buttons(TopButtonsPickedSection::Playlists).map(|msg| msg.into()),
            container(row![
                column![
                    column![
                        text(format!("Edituješ playlist \"{}\"", playlist_name)),
                        button("Uložit")
                            .on_press_maybe(save_button_msg)
                            .width(Length::Fill),
                        row![
                            text_input("Název nového playlistu", "")
                                .on_input(Message::NewPlaylistNameChanged),
                            button("Uložit jako").on_press(Message::SavePlaylistAsClicked)
                        ]
                        .width(Length::Fill),
                        text(&self.new_playlist_err_msg)
                            .style(text::danger)
                            .width(Length::Fill),
                        button("Smazat playlist")
                            .style(button::danger)
                            .on_press_maybe(delete_button_msg)
                            .width(Length::Fill),
                        button("Přidat píseň")
                            .on_press(Message::AddSong)
                            .width(Length::Fill),
                        button("Přidat verše")
                            .on_press(Message::AddBiblePassage)
                            .width(Length::Fill),
                        button("Prezentovat")
                            .on_press(Message::StartPresentation)
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
                column!["Tady budou položky"].width(Length::FillPortion(2)),
                column([]).width(Length::FillPortion(1))
            ])
            .padding(10)
            .center_x(Length::FillPortion(1))
        ])
        .explain(Color::BLACK)
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
                        let conn = conn.await.context("Nelze získat připojení k databázi")?;
                        let mut playlist = playlist.lock().await;
                        playlist.save(conn).await
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
                let playlist = editor.playlist.clone();
                Task::perform(
                    async move {
                        let mut playlist = playlist.lock().await;

                        *playlist = playlist::PlaylistMetadata::from_other(
                            &new_playlist_name,
                            &mut playlist,
                        );

                        let conn = conn.await.context("Nelze získat připojení k databázi")?;
                        playlist.save(conn).await
                    },
                    |res| match res {
                        Ok(_) => Message::PlaylistSavedSuccessfully.into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::StartPresentation => todo!(),
            Message::AddBiblePassage => todo!(),
            Message::AddSong => todo!(),
            Message::PlaylistSavedSuccessfully => {
                debug!("Playlist byl úspéšně uložen");
                editor.is_saved = true;
                editor.new_playlist_err_msg.clear();
                Task::none()
            }
            Message::SavePlaylistAsClicked => Task::done(Message::ValidateNewPlaylistName.into()),
            Message::ValidateNewPlaylistName => {
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
                                Message::NewPlaylistNameTaken.into()
                            }
                        }
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::NewPlaylistNameTaken => {
                debug!("Nastavuji chybovou hlášku, aby uživatel změnil název nového playlistu");
                editor.new_playlist_err_msg =
                    String::from("Playlist s daným názvem již existuje, vyber jiný");
                Task::none()
            }
            Message::TopButtonsPlaylist => todo!(),
            Message::TopButtonsSongs => {
                todo!("Ještě neumím editovat písně")
            }
            Message::NewPlaylistNameChanged(input) => {
                trace!("Změnil se nový název playlistu: {input}");
                editor.new_playlist_name = input;
                Task::none()
            }
            Message::DeletePlaylist => {
                {
                    debug!(
                        "Mažu playlist \"{}\"",
                        editor.playlist.blocking_lock().get_name()
                    )
                }

                panic!("Ještě neumím mazat playlisty")
                // Asi bych měl jít zpátky na obrazovku výběru playlistu
            }
            Message::ReturnToPlaylistPicker => {
                state.screen = Screen::PickPlaylist(PlaylistPicker::default());
                Task::none()
            }
            Message::SaveAndExit => {
                let playlist_status = { editor.playlist.blocking_lock().get_status() };

                match playlist_status {
                    PlaylistMetadataStatus::Transient | PlaylistMetadataStatus::Dirty(_) => {
                        debug!("Ukládám playlist a vracím se k výběru playlistů");
                        let conn = state.db.acquire();
                        let playlist = editor.playlist.clone();
                        Task::perform(
                            async move {
                                let conn =
                                    conn.await.context("Nelze získat připojení k databázi")?;
                                let mut playlist = playlist.lock().await;
                                playlist.save(conn).await
                            },
                            |res| res,
                        )
                        .then(|res| {
                            debug!("Playlist uložen, vracím se na výběr playlistů");
                            match res {
                                Ok(_) => Task::batch([
                                    Task::done(Message::ReturnToPlaylistPicker.into()),
                                    Task::done(crate::pick_playlist::Message::LoadPlaylists.into()),
                                ]),
                                Err(e) => Task::done(crate::Message::FatalErrorOccured(format!(
                                    "{:?}",
                                    e
                                ))),
                            }
                        })
                    }
                    PlaylistMetadataStatus::Clean(_) => {
                        debug!("Playlist nepotřebuje uložit, vracím se rovnou na výběr playlistů");
                        Task::batch([
                            Task::done(Message::ReturnToPlaylistPicker.into()),
                            Task::done(crate::pick_playlist::Message::LoadPlaylists.into()),
                        ])
                    }
                }
            }
        }
    }
}
