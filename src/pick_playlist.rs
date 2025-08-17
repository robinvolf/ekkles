use std::fmt::Display;

use crate::components::{TopButtonsMessage, top_buttons};
use iced::{
    Element, Length,
    widget::{column, combo_box, container, text},
};

#[derive(Debug)]
pub struct PlaylistPicker {
    pub playlists: Option<combo_box::State<PlaylistPickerItem>>,
    pub picked_playlist: Option<PlaylistPickerItem>,
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
    PlaylistsLoaded(Vec<(i64, String)>),
    PickedPlaylist,
}

impl PlaylistPicker {
    pub fn new() -> Self {
        Self {
            playlists: None,
            picked_playlist: None,
        }
    }

    pub fn view(&self) -> Element<Message> {
        let box_with_playlists = if self.playlists.is_some() {
            Into::<Element<Message>>::into(combo_box(
                self.playlists.as_ref().unwrap(),
                "Vyber playlist...",
                self.picked_playlist.as_ref(),
                |_| Message::PickedPlaylist,
            ))
        } else {
            text("Načítám playlisty z databáze").into()
        };

        column![
            top_buttons(crate::components::TopButtonsPickedSection::Playlists)
                .map(|msg| msg.into()),
            container(column!("Vyber playlist", box_with_playlists))
                .padding(20)
                .height(Length::Fill)
                .width(Length::Fill)
        ]
        .into()
    }
}
