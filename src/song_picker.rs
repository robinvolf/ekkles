use std::{fmt::Display, time::Duration};

use anyhow::Result;
use ekkles_data::{Song, bible::get_available_translations, playlist::PlaylistMetadata};
use iced::{
    Alignment, Color, Element, Length, Task,
    widget::{button, column, combo_box, container, row, text},
};
use log::debug;
use sqlx::{Sqlite, pool::PoolConnection};
use tokio::time::sleep;

use crate::{Ekkles, Screen, playlist_editor::PlaylistEditor};

#[derive(Debug, Clone)]
pub struct SongPickerItem {
    id: i64,
    name: String,
}

impl Display for SongPickerItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

impl SongPickerItem {
    fn new(id: i64, name: String) -> Self {
        Self { id, name }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    LoadSongs,
    SongsLoaded(Vec<SongPickerItem>),
    ReturnToEditor,
    SongPicked(i64),
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::SongPicker(value)
    }
}

#[derive(Debug)]
pub struct SongPicker {
    songs: Option<combo_box::State<SongPickerItem>>,
    picked_song: Option<SongPickerItem>,
    playlist: PlaylistMetadata,
}

impl SongPicker {
    pub fn new(playlist: PlaylistMetadata) -> Self {
        Self {
            songs: None,
            picked_song: None,
            playlist,
        }
    }

    pub async fn load_song_list(conn: &mut PoolConnection<Sqlite>) -> Result<Vec<SongPickerItem>> {
        Song::get_available_from_db(conn).await.map(|vec| {
            vec.into_iter()
                .map(|(id, name)| SongPickerItem::new(id, name))
                .collect()
        })
    }

    pub fn set_song_list(&mut self, song_list: Vec<SongPickerItem>) {
        self.songs = Some(combo_box::State::new(song_list));
    }

    pub fn view(&self) -> Element<Message> {
        let picker: Element<Message> = self
            .songs
            .as_ref()
            .map(|combo_box_state| {
                combo_box(combo_box_state, "Název písně", None, |item| {
                    Message::SongPicked(item.id)
                })
                .into()
            })
            .unwrap_or(text("Načítám písně ...").into());

        Into::<Element<Message>>::into(container(
            row![
                container(
                    button("Zpět")
                        .on_press(Message::ReturnToEditor)
                        .width(Length::Fill)
                )
                .align_bottom(Length::Fill)
                .width(Length::FillPortion(1))
                .padding(30),
                column![text("Vyber píseň:"), picker]
                    .spacing(10)
                    .align_x(Alignment::Center)
                    .width(Length::FillPortion(2)),
                container("").width(Length::FillPortion(1))
            ]
            .padding(10)
            .height(Length::Fill)
            .align_y(Alignment::Center),
        ))
        // .explain(Color::BLACK)
    }

    pub fn update(state: &mut Ekkles, message: Message) -> Task<crate::Message> {
        let picker = match &mut state.screen {
            Screen::PickSong(picker) => picker,
            screen => panic!(
                "Update pro PickPlaylist zavolán, nad obrazovkou {:#?}",
                screen
            ),
        };

        match message {
            Message::LoadSongs => {
                debug!("Načítám seznam písní");
                let conn = state.db.acquire();
                Task::perform(
                    async {
                        let mut conn = conn.await?;
                        SongPicker::load_song_list(&mut conn).await
                    },
                    |res| match res {
                        Ok(songs) => Message::SongsLoaded(songs).into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::SongsLoaded(song_picker_items) => {
                debug!("Písně načteny: {:#?}", &song_picker_items);
                picker.set_song_list(song_picker_items);
                Task::none()
            }
            Message::ReturnToEditor => {
                debug!("Vracím se do editoru");
                state.screen = Screen::EditPlaylist(PlaylistEditor::new(picker.playlist.clone()));
                Task::done(crate::playlist_editor::Message::LoadSongNameCache.into())
            }
            Message::SongPicked(id) => {
                debug!("Byla vybrána píseň s id {id}");
                picker.playlist.push_song(id);
                Task::done(Message::ReturnToEditor.into())
            }
        }
    }
}
