use std::{collections::HashMap, fmt::Display};

use crate::{
    Ekkles, Screen,
    components::{LazyLoadable, LazyLoadableState},
    playlist_editor,
};
use anyhow::Context;
use ekkles_data::{
    Song,
    playlist::{self, PlaylistMetadata},
};
use iced::{
    Element, Length, Task,
    widget::{button, column, combo_box, container, row, space, text, text::danger, text_input},
};
use log::{debug, trace};

#[derive(Debug)]
pub struct StartScreen {
    playlists: LazyLoadable<combo_box::State<StartScreenPickerBoxItem>, Message>,
    songs: LazyLoadable<combo_box::State<StartScreenPickerBoxItem>, Message>,
    new_name: String,
    err_msg: Option<String>,
    tab: Tab,
}

#[derive(Debug, Clone, Copy)]
enum Tab {
    Song,
    Playlist,
}

#[derive(Debug, Clone)]
pub struct StartScreenPickerBoxItem {
    id: i64,
    name: String,
}

impl Display for StartScreenPickerBoxItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::PlaylistPicker(value)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    TabSongs,
    TabPlaylists,
    LoadPlaylists,
    PlaylistsLoaded(Vec<(i64, String)>),
    LoadSongs,
    SongsLoaded(Vec<(i64, String)>),
    PickedPlaylist(i64),
    PickedSong(i64),
    NewNameChanged(String),
    ValidateNewName,
    CreateNew,
    NameAlreadyTaken,
    EditPlaylist(PlaylistMetadata),
    EditSong(Song),
}

/// Update funkce pro PickPlaylist. Pokud bude zavolána na jiné obrazovce, zpanikaří.
pub fn update(state: &mut Ekkles, msg: Message) -> Task<crate::Message> {
    let picker = if let Screen::PickEditor(picker) = &mut state.screen {
        picker
    } else {
        panic!("Update pro PickPlaylist zavolána na jinou obrazovku");
    };

    match msg {
        Message::TabSongs => {
            debug!("Nastavuji na výběr písně");
            picker.tab = Tab::Song;
            Task::none()
        }
        Message::TabPlaylists => {
            debug!("Nastavuji na výběr playlistu");
            picker.tab = Tab::Playlist;
            Task::none()
        }
        Message::PlaylistsLoaded(playlists) => {
            debug!("Načetlo se {} playlistů", playlists.len());
            let options = playlists
                .into_iter()
                .map(|(id, name)| StartScreenPickerBoxItem { id, name })
                .collect();
            picker
                .playlists
                .finish_loading(combo_box::State::new(options));
            Task::none()
        }
        Message::PickedPlaylist(id) => {
            debug!("Byl vybrán playlist k otevření, jdu ho načíst z databáze");

            let conn = state.db.acquire();
            let picked_playlist_id = id;

            Task::perform(
                async move {
                    let conn = conn.await.context("Nelze získat připojení k databázi")?;
                    PlaylistMetadata::load(picked_playlist_id, conn).await
                },
                |res| match res {
                    Ok(loaded_playlist) => Message::EditPlaylist(loaded_playlist).into(),
                    Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                },
            )
            .chain(Task::done(
                crate::playlist_editor::Message::LoadSongNameCache.into(),
            ))
        }
        Message::NewNameChanged(input) => {
            trace!("Změnil se textový vstup pro název nového playlistu");
            picker.new_name = input;
            Task::none()
        }
        Message::CreateNew => {
            let name = picker.new_name.trim();
            match picker.tab {
                Tab::Song => {
                    debug!("Vytvářím novou píseň \"{}\"", name);
                    let new_song = Song {
                        title: name.to_string(),
                        author: None,
                        parts: HashMap::new(),
                        order: Vec::new(),
                    };
                    Task::done(Message::EditSong(new_song).into())
                }
                Tab::Playlist => {
                    debug!("Vytvářím nový playlist \"{}\"", name);
                    let new_playlist = PlaylistMetadata::new(name);
                    Task::done(Message::EditPlaylist(new_playlist).into())
                }
            }
        }
        Message::ValidateNewName => {
            debug!("Zjišťuji, jestli se v databázi nachází položka s daným názvem");
            let conn = state.db.acquire();
            let name = picker.new_name.clone();
            let tab = picker.tab;
            Task::perform(
                async move {
                    let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                    match tab {
                        Tab::Song => Song::exists_in_db(&mut conn, &name)
                            .await
                            .map(|o| o.is_none()),
                        Tab::Playlist => playlist::is_name_available(conn, &name).await,
                    }
                },
                |res| match res {
                    Ok(available) => {
                        if available {
                            Message::CreateNew.into()
                        } else {
                            Message::NameAlreadyTaken.into()
                        }
                    }
                    Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                },
            )
        }
        Message::NameAlreadyTaken => {
            let item_name = match picker.tab {
                Tab::Song => "Píseň",
                Tab::Playlist => "Playlist",
            };

            debug!(
                "{item_name} s daným názvem existuje, nic nevytvářím a nastavuju chybovou hlášku"
            );

            let err_msg = format!(
                "{} s názvem \"{}\" již existuje, vyber jiný název",
                item_name, picker.new_name
            );
            picker.err_msg = Some(err_msg);
            Task::none()
        }
        Message::EditPlaylist(playlist) => {
            debug!("Vybrán playlist, přecházím na editaci {:#?}", playlist);
            state.screen = Screen::EditPlaylist(playlist_editor::PlaylistEditor::new(playlist));
            Task::none()
        }
        Message::LoadPlaylists => {
            debug!("Načítám seznam playlistů");
            // Vyrobíme future, kterou awaitneme v asynchronním bloku v Perform a ta nám vydá connection
            picker.playlists.start_loading();
            let conn = state.db.acquire();
            Task::perform(
                async move {
                    let conn = conn.await.context("Nelze získat připojení k databázi")?;
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    playlist::get_available(conn).await
                },
                |res| match res {
                    Ok(pls) => Message::PlaylistsLoaded(pls).into(),
                    Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                },
            )
        }
        Message::LoadSongs => {
            debug!("Načítám seznam písní");
            picker.songs.start_loading();
            let conn = state.db.acquire();
            Task::perform(
                async move {
                    let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                    Song::get_available_from_db(&mut conn).await
                },
                |res| match res {
                    Ok(pls) => Message::SongsLoaded(pls).into(),
                    Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                },
            )
        }
        Message::SongsLoaded(songs) => {
            debug!("Načetlo se {} písní", songs.len());
            let options = songs
                .into_iter()
                .map(|(id, name)| StartScreenPickerBoxItem { id, name })
                .collect();
            picker.songs.finish_loading(combo_box::State::new(options));
            Task::none()
        }
        Message::PickedSong(id) => {
            debug!("Byla vybrána píseň k otevření, jdu ji načíst z databáze");

            let conn = state.db.acquire();
            let picked_song_id = id;

            Task::perform(
                async move {
                    let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                    Song::load_from_db(picked_song_id, &mut conn).await
                },
                |res| match res {
                    Ok(loaded_song) => Message::EditSong(loaded_song).into(),
                    Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                },
            )
        }
        Message::EditSong(_) => {
            todo!("Ještě neumím editovat písně :(");
            // debug!("Vybrána píseň, přecházím na editaci {:#?}", song);
            // state.screen = ...
            // Task::none()
        }
    }
}

impl StartScreen {
    pub fn new() -> Self {
        Self {
            playlists: LazyLoadable::new(Message::LoadPlaylists),
            songs: LazyLoadable::new(Message::LoadSongs),
            new_name: String::from(""),
            err_msg: None,
            tab: Tab::Playlist,
        }
    }

    pub fn view(&self) -> Element<Message> {
        let (
            new_name_description,
            pick_existing_description,
            create_new_description,
            tab_song_msg,
            tab_playlist_msg,
        ) = match self.tab {
            Tab::Song => (
                "Název nové písně:",
                "Vyber píseň",
                "Nebo vytvoř novou",
                None,
                Some(Message::TabPlaylists),
            ),
            Tab::Playlist => (
                "Název nového playlistu:",
                "Vyber playlist",
                "Nebo vytvoř nový",
                Some(Message::TabSongs),
                None,
            ),
        };

        let data_src = match self.tab {
            Tab::Song => &self.songs,
            Tab::Playlist => &self.playlists,
        };

        let central_column = match data_src.state() {
            LazyLoadableState::Cold | LazyLoadableState::Loading => {
                let picker_not_loaded = data_src.view_not_loaded();
                column![
                    space().height(Length::FillPortion(1)),
                    picker_not_loaded.height(Length::Shrink),
                    space().height(Length::FillPortion(1)),
                ]
            }
            LazyLoadableState::Loaded(picker_state) => {
                let picker_loaded = match self.tab {
                    Tab::Song => combo_box(picker_state, "Název písně", None, |picked| {
                        Message::PickedSong(picked.id)
                    }),
                    Tab::Playlist => combo_box(picker_state, "Název playlistu", None, |picked| {
                        Message::PickedPlaylist(picked.id)
                    }),
                };
                column![
                    space().height(Length::FillPortion(1)),
                    pick_existing_description,
                    picker_loaded,
                    create_new_description,
                    row![
                        text_input(new_name_description, &self.new_name)
                            .on_input(|input| Message::NewNameChanged(input))
                            .on_submit(Message::ValidateNewName),
                        button("Vytvořit!").on_press(Message::ValidateNewName),
                    ]
                    .spacing(10),
                    text(self.err_msg.clone().unwrap_or(String::from(""))).style(danger),
                    space().height(Length::FillPortion(1)),
                ]
            }
        };

        Into::<Element<Message>>::into(column![
            row![
                button("Editovat Píseň")
                    .on_press_maybe(tab_song_msg)
                    .width(Length::FillPortion(1)),
                button("Editovat Playlist")
                    .on_press_maybe(tab_playlist_msg)
                    .width(Length::FillPortion(1)),
            ]
            .width(Length::Fill),
            container(row![
                space().width(Length::FillPortion(1)),
                central_column
                    .spacing(10)
                    .width(Length::FillPortion(1))
                    .max_width(1000),
                space().width(Length::FillPortion(1))
            ])
            .padding(10)
            .center_x(Length::FillPortion(1))
            .center_y(Length::Fill),
        ])
    }
}

impl Default for StartScreen {
    fn default() -> Self {
        Self::new()
    }
}
