use std::fmt::Display;

use crate::{
    Ekkles, Screen,
    components::{TopButtonsMessage, top_buttons},
    playlist_editor,
};
use ekkles_data::playlist::PlaylistMetadata;
use iced::{
    Element, Length, Task,
    widget::{button, column, combo_box, container, horizontal_rule, row, text, text_input},
};
use log::debug;

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
        Message::PickedPlaylist => {
            debug!("Byl vybrán playlist k otevření");
            todo!("Ještě neumím editovat playlisty")
        }
        Message::NewPlaylistNameChanged(input) => {
            picker.new_playlist_name = input;
            Task::none()
        }
        Message::CreateNewPlaylist => {
            let new_playlist = PlaylistMetadata::new(picker.new_playlist_name.trim());
            debug!("Vytvářím nový playlist \"{}\"", new_playlist.get_name());
            state.screen = Screen::EditPlaylist(playlist_editor::PlaylistEditor::new(new_playlist));
            Task::none()
        }
    }
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
