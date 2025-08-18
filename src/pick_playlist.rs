use std::fmt::Display;

use crate::components::{TopButtonsMessage, top_buttons};
use iced::{
    Element, Length,
    widget::{button, column, combo_box, container, horizontal_rule, row, text, text_input},
};

#[derive(Debug)]
pub struct PlaylistPicker {
    pub playlists: Option<combo_box::State<PlaylistPickerItem>>,
    pub picked_playlist: Option<PlaylistPickerItem>,
    pub new_playlist_name: String,
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
    NewPlaylistNameChanged(String),
    CreateNewPlaylist,
}

impl PlaylistPicker {
    pub fn new() -> Self {
        Self {
            playlists: None,
            picked_playlist: None,
            new_playlist_name: String::from(""),
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
            container(
                column![
                    column!["Vyber playlist", box_with_playlists].spacing(10),
                    column![
                        "Nebo vytvoř nový",
                        row![
                            text_input("Název nového playlistu", &self.new_playlist_name)
                                .on_input(|input| Message::NewPlaylistNameChanged(input))
                                .on_submit(Message::CreateNewPlaylist),
                            button("Vytvořit!").on_press(Message::CreateNewPlaylist),
                        ]
                        .spacing(10)
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
