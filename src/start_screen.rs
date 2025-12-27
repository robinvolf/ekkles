use std::collections::HashMap;

use crate::{
    Ekkles, Screen,
    components::{
        LazyLoadable, LazyLoadableState, PickerItem,
        song_picker::{self, SongPicker},
    },
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
    playlists: LazyLoadable<combo_box::State<PickerItem>, Message>,
    song_picker: SongPicker,
    new_name: String,
    err_msg: Option<String>,
    tab: Tab,
}

#[derive(Debug, Clone, Copy)]
enum Tab {
    Song,
    Playlist,
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
    SongPickerMsg(song_picker::Message),
    LoadPlaylists,
    PlaylistsLoaded(Vec<(i64, String)>),
    PickedPlaylist(i64),
    NewNameChanged(String),
    ValidateNewName,
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
                .map(|(id, name)| PickerItem { id, name })
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
        Message::ValidateNewName => {
            debug!("Validuji název nové položky");
            let others = match picker.tab {
                Tab::Song => picker.song_picker.songs().state(),
                Tab::Playlist => picker.playlists.state(),
            }
            .as_loaded()
            .expect("Validace jména může proběhnout až potom, co jsou načtena data z databáze")
            .options()
            .iter()
            .map(|item| &item.name);

            let new_name = picker.new_name.trim();

            // Toto je potenciálně drahá operace, tudíž by šlo přesunout ji do Task, aby se vypočetla na pozadí a neblokovala GUI
            // Ovšem neočekáváme příliš dlouhý výpočet (počet položek v řádu maximálně tisíců)
            let is_unique = is_new_name_unique(&new_name, others);

            if is_unique {
                match picker.tab {
                    Tab::Song => {
                        debug!("Vytvářím novou píseň \"{}\"", new_name);
                        let new_song = Song {
                            title: new_name.to_string(),
                            author: None,
                            parts: HashMap::new(),
                            order: Vec::new(),
                        };
                        Task::done(Message::EditSong(new_song).into())
                    }
                    Tab::Playlist => {
                        debug!("Vytvářím nový playlist \"{}\"", new_name);
                        let new_playlist = PlaylistMetadata::new(new_name);
                        Task::done(Message::EditPlaylist(new_playlist).into())
                    }
                }
            } else {
                debug!(
                    "Položka s daným názvem existuje, nic nevytvářím a nastavuju chybovou hlášku"
                );

                let err_msg = format!(
                    "Položka s názvem \"{}\" již existuje, vyber jiný název",
                    new_name
                );
                picker.err_msg = Some(err_msg);
                Task::none()
            }
        }
        Message::EditPlaylist(playlist) => {
            debug!("Vybrán playlist, přecházím na editaci {:#?}", playlist);
            state.screen = Screen::EditPlaylist(playlist_editor::PlaylistEditor::new(playlist));
            Task::none()
        }
        Message::LoadPlaylists => {
            debug!("Načítám seznam playlistů");
            // Vyrobíme future, kterou awaitneme v asynchronním bloku v Perform a ta nám vydá connection
            let conn = state.db.acquire();
            let (task, handle) = Task::abortable(Task::perform(
                async move {
                    let conn = conn.await.context("Nelze získat připojení k databázi")?;
                    playlist::get_available(conn).await
                },
                |res| match res {
                    Ok(pls) => Message::PlaylistsLoaded(pls).into(),
                    Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                },
            ));
            picker.playlists.start_loading(handle);
            task
        }
        Message::EditSong(song) => {
            debug!("Vybrána píseň, přecházím na editaci {:#?}", song);
            todo!("Ještě neumím editovat písně :(");
            // state.screen = ...
            // Task::none()
        }
        Message::SongPickerMsg(msg) => match msg {
            song_picker::Message::Return => Task::done(crate::Message::ShouldQuit),
            song_picker::Message::ReturnSelected(picker_item) => {
                let conn = state.db.acquire();
                Task::perform(
                    async move {
                        let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                        Song::load_from_db(picker_item.id, &mut conn).await
                    },
                    |res| match res {
                        Ok(song) => Message::EditSong(song).into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            song_picker::Message::FatalError(e) => Task::done(crate::Message::FatalErrorOccured(e)),
            _ => picker
                .song_picker
                .update(&state.db, msg)
                .map(|m| Message::SongPickerMsg(m).into()),
        },
    }
}

impl StartScreen {
    pub fn new() -> Self {
        Self {
            playlists: LazyLoadable::new(Message::LoadPlaylists),
            song_picker: SongPicker::new(),
            new_name: String::from(""),
            err_msg: None,
            tab: Tab::Playlist,
        }
    }

    pub fn view(&self) -> Element<Message> {
        let central_column = match self.tab {
            Tab::Song => {
                let song_picker = self.song_picker.view();
                column![
                    space().height(Length::FillPortion(1)),
                    "Vyber píseň",
                    container(song_picker.map(Message::SongPickerMsg)).height(Length::Fixed(500.0)),
                    "Nebo vytvoř novou píseň",
                    row![
                        text_input("Název nové písně", &self.new_name)
                            .on_input(|input| Message::NewNameChanged(input))
                            .on_submit(Message::ValidateNewName),
                        button("Vytvořit!").on_press(Message::ValidateNewName),
                    ]
                    .spacing(10),
                    text(self.err_msg.clone().unwrap_or(String::from(""))).style(danger),
                    space().height(Length::FillPortion(1)),
                ]
            }
            Tab::Playlist => {
                let playlist_picker = match self.playlists.state() {
                    LazyLoadableState::Cold | LazyLoadableState::Loading(_) => {
                        let picker_not_loaded = self.playlists.view_not_loaded();
                        column![
                            space().height(Length::FillPortion(1)),
                            picker_not_loaded.height(Length::Shrink),
                            space().height(Length::FillPortion(1)),
                        ]
                    }
                    LazyLoadableState::Loaded(picker_state) => {
                        column![
                            space().height(Length::FillPortion(1)),
                            combo_box(picker_state, "Název playlistu", None, |picked| {
                                Message::PickedPlaylist(picked.id)
                            }),
                            space().height(Length::FillPortion(1)),
                        ]
                    }
                };

                column![
                    space().height(Length::FillPortion(1)),
                    "Vyber playlist",
                    playlist_picker,
                    "Nebo vytvoř nový playlist",
                    row![
                        text_input("Název nového playlistu", &self.new_name)
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

        let (tab_song_msg, tab_playlist_msg) = match self.tab {
            Tab::Song => (None, Some(Message::TabPlaylists)),
            Tab::Playlist => (Some(Message::TabSongs), None),
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
                    .height(Length::Shrink)
                    .width(Length::FillPortion(2))
                    .max_width(1000),
                space().width(Length::FillPortion(1))
            ])
            .padding(10)
            .center_x(Length::FillPortion(1))
            .center_y(Length::Fill),
        ])
    }
}

/// Zkontroluje zda-li je `name` unikátní (že se `name` mezi `others` nevyskytuje)
fn is_new_name_unique(name: &str, others: impl IntoIterator<Item: AsRef<str>>) -> bool {
    others
        .into_iter()
        .find(|item| item.as_ref() == name)
        .is_none()
}

impl Default for StartScreen {
    fn default() -> Self {
        Self::new()
    }
}
