use std::fmt::Display;

use anyhow::Result;
use ekkles_data::{Song, playlist::PlaylistMetadata};
use iced::{
    Alignment, Element, Length, Task,
    widget::{
        Container, Space, button, column, combo_box, container, horizontal_rule, row, scrollable,
        text,
    },
};
use log::debug;
use sqlx::{Sqlite, pool::PoolConnection};

use crate::{Ekkles, Screen, components::Preview, playlist_editor::PlaylistEditor};

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
    SelectSong(SongPickerItem),
    LoadPreview(SongPickerItem),
    PreviewLoaded(Song),
    ConfirmSelection,
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::SongPicker(value)
    }
}

#[derive(Debug)]
pub struct SongPicker {
    songs: Option<combo_box::State<SongPickerItem>>,
    selected: Option<SongPickerItem>,
    playlist: PlaylistMetadata,
    preview: Preview<Song>,
}

impl SongPicker {
    pub fn new(playlist: PlaylistMetadata) -> Self {
        Self {
            songs: None,
            playlist,
            preview: Preview::new(),
            selected: None,
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
        let picker = self
            .songs
            .as_ref()
            .map(|combo_box_state| {
                container(
                    combo_box(
                        combo_box_state,
                        "Název písně",
                        self.selected.as_ref(),
                        Message::SelectSong,
                    )
                    .on_option_hovered(Message::LoadPreview),
                )
            })
            .unwrap_or(container(text("Načítám písně ...")));

        let preview = match &self.preview {
            Preview::Empty => container(Space::new(Length::Shrink, Length::Shrink)),
            Preview::Loading(_) => container(text("Načítám náhled ...")).center(Length::Fill),
            Preview::Loaded(song) => song_preview(song),
        };

        Into::<Element<Message>>::into(container(
            row![
                column![
                    picker.height(Length::Fill),
                    button("Zpět")
                        .on_press(Message::ReturnToEditor)
                        .width(Length::Fill)
                        .height(Length::Shrink)
                ],
                column![
                    preview.height(Length::Fill),
                    button("Potvrdit")
                        .width(Length::Fill)
                        .height(Length::Shrink)
                        .on_press(Message::ConfirmSelection)
                ]
            ]
            .padding(30)
            .spacing(10),
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
            Message::ConfirmSelection => {
                debug!("Byla potvrzena píseň {:?}", picker.selected);
                let id = picker.selected.as_ref().expect(
                            "Byla potvrzena volba, ale nebyla zvolena píseň, toto by se nikdy nemělo stát",
                        ).id;
                picker.playlist.push_song(id);
                Task::done(Message::ReturnToEditor.into())
            }
            Message::LoadPreview(item) => {
                debug!("Načítám preview pro píseň {}", item.name);
                let conn = state.db.acquire();
                let fut = async move {
                    let mut conn = conn.await?;
                    Song::load_from_db(item.id, &mut conn).await
                };
                picker.preview.load(fut).map(|res| match res {
                    Ok(song) => Message::PreviewLoaded(song).into(),
                    Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                })
            }
            Message::PreviewLoaded(song) => {
                debug!("Načetlo se previw pro píseň {}", song.title);
                picker.preview.loaded(song);
                Task::none()
            }
            Message::SelectSong(selected) => {
                debug!("Byla vybrána píseň {:?}", picker.selected);
                picker.selected = Some(selected);
                Task::none()
            }
        }
    }
}

/// Vytvoří preview písně
fn song_preview(song: &Song) -> Container<'static, Message> {
    let lyrics: String = song
        .order
        .iter()
        .map(|part| format!("[{}]\n{}\n", part, song.parts.get(part).unwrap()))
        .collect();

    container(column![
        text(song.title.clone()).center(),
        horizontal_rule(2),
        container(scrollable(text(lyrics).align_x(Alignment::Center))).center(Length::Fill),
    ])
}
