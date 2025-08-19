use std::fmt::Display;

use crate::{
    Ekkles, Screen,
    components::{TopButtonsMessage, top_buttons},
    playlist_editor,
};
use anyhow::Context;
use ekkles_data::playlist::{self, PlaylistMetadata};
use iced::{
    Element, Length, Task,
    widget::{button, column, combo_box, container, row, text, text::danger, text_input},
};
use log::{debug, trace};

#[derive(Debug)]
pub struct PlaylistPicker {
    pub playlists: Option<combo_box::State<PlaylistPickerItem>>,
    pub picked_playlist: Option<PlaylistPickerItem>,
    pub new_playlist_name: String,
    pub err_msg: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlaylistPickerItem {
    pub id: i64,
    pub name: String,
}

impl Display for PlaylistPickerItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl From<TopButtonsMessage> for Message {
    fn from(value: TopButtonsMessage) -> Self {
        match value {
            TopButtonsMessage::Playlists => Message::TopButtonPlaylists,
            TopButtonsMessage::Songs => Message::TopButtonSongs,
        }
    }
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::PlaylistPicker(value)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    TopButtonSongs,
    TopButtonPlaylists,
    LoadPlaylists,
    PlaylistsLoaded(Vec<(i64, String)>),
    PickedPlaylist(i64),
    NewPlaylistNameChanged(String),
    CreateNewPlaylist,
    ValidateNewPlaylistName,
    NameAlreadyTaken,
    EditPlaylist(PlaylistMetadata),
}

/// Update funkce pro PickPlaylist. Pokud bude zavolána na jiné obrazovce, zpanikaří.
pub fn update(state: &mut Ekkles, msg: Message) -> Task<crate::Message> {
    let picker = if let Screen::PickPlaylist(picker) = &mut state.screen {
        picker
    } else {
        panic!("Update pro PickPlaylist zavolána na jinou obrazovku");
    };

    match msg {
        Message::TopButtonSongs => {
            todo!("Ještě neumím editovat písně")
        }
        Message::TopButtonPlaylists => {
            debug!("Jsem v playlistu a klikám, abych se do něj znovu dostal, ignoruju");
            Task::none()
        }
        Message::PlaylistsLoaded(playlists) => {
            debug!("Načetly se playlisty");
            let options = playlists
                .into_iter()
                .map(|(id, name)| PlaylistPickerItem { id, name })
                .collect();
            picker.playlists = Some(iced::widget::combo_box::State::new(options));
            Task::none()
        }
        Message::PickedPlaylist(id) => {
            debug!("Byl vybrán playlist k otevření, jdu ho načíst z databáze");

            // todo!("Ještě neumím editovat playlisty");
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
        }
        Message::NewPlaylistNameChanged(input) => {
            trace!("Změnil se textový vstup pro název nového playlistu");
            picker.new_playlist_name = input;
            Task::none()
        }
        Message::CreateNewPlaylist => {
            let name = picker.new_playlist_name.trim();
            debug!("Vytvářím nový playlist \"{}\"", name);
            let new_playlist = PlaylistMetadata::new(name);
            Task::done(Message::EditPlaylist(new_playlist).into())
        }
        Message::ValidateNewPlaylistName => {
            debug!("Zjišťuji, jestli se v databázi nachází playlist s daným názvem");
            let conn = state.db.acquire();
            let name = picker.new_playlist_name.clone();
            Task::perform(
                async move {
                    let conn = conn.await.context("Nelze získat připojení k databázi")?;
                    playlist::is_name_available(conn, &name).await
                },
                |res| match res {
                    Ok(available) => {
                        if available {
                            Message::CreateNewPlaylist.into()
                        } else {
                            Message::NameAlreadyTaken.into()
                        }
                    }
                    Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                },
            )
        }
        Message::NameAlreadyTaken => {
            debug!("Playlist s daným názvem existuje, nic nevytvářím a nastavuju chybovou hlášku");
            picker.err_msg = Some(format!(
                "Playlist s názvem \"{}\" již existuje, vyber jiný název",
                picker.new_playlist_name
            ));
            Task::none()
        }
        Message::EditPlaylist(playlist) => {
            debug!("Vybrán playlist, přecházím na editaci {:#?}", playlist);
            state.screen = Screen::EditPlaylist(playlist_editor::PlaylistEditor::new(playlist));
            Task::none()
        }
        Message::LoadPlaylists => {
            debug!("Načítám seznam playlistů pro výběr playlistů");
            // Vyrobíme future, kterou awaitneme v asynchronním bloku v Perform a ta nám vydá connection
            let conn = state.db.acquire();
            Task::perform(
                async move {
                    let conn = conn.await.context("Nelze získat připojení k databázi")?;
                    playlist::get_available(conn).await
                },
                |res| match res {
                    Ok(pls) => Message::PlaylistsLoaded(pls).into(),
                    Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                },
            )
        }
    }
}

impl PlaylistPicker {
    pub fn new() -> Self {
        Self {
            playlists: None,
            picked_playlist: None,
            new_playlist_name: String::from(""),
            err_msg: None,
        }
    }

    pub fn view(&self) -> Element<Message> {
        let box_with_playlists = if self.playlists.is_some() {
            Into::<Element<Message>>::into(combo_box(
                self.playlists.as_ref().unwrap(),
                "Vyber playlist...",
                self.picked_playlist.as_ref(),
                |picked| Message::PickedPlaylist(picked.id),
            ))
        } else {
            text("Načítám playlisty z databáze").into()
        };

        column![
            top_buttons(crate::components::TopButtonsPickedSection::Playlists)
                .map(|msg| msg.into()),
            container(
                column![
                    column!["Vyber playlist", box_with_playlists].spacing(10),
                    column![
                        "Nebo vytvoř nový",
                        row![
                            text_input("Název nového playlistu", &self.new_playlist_name)
                                .on_input(|input| Message::NewPlaylistNameChanged(input))
                                .on_submit(Message::ValidateNewPlaylistName),
                            button("Vytvořit!").on_press(Message::ValidateNewPlaylistName),
                        ]
                        .spacing(10),
                        text(self.err_msg.clone().unwrap_or(String::from(""))).style(danger)
                    ]
                    .spacing(10)
                ]
                .spacing(30)
                .max_width(1000)
            )
            .padding(10)
            .center_x(Length::FillPortion(1))
            .center_y(Length::Fill),
        ]
        .into()
    }
}

impl Default for PlaylistPicker {
    fn default() -> Self {
        Self::new()
    }
}
