use std::fmt::Display;

use anyhow::Result;
use ekkles_data::{Song, playlist::PlaylistMetadata};
use iced::{
    Alignment, Color, Element, Length, Task,
    task::Handle,
    widget::{Container, Space, button, column, combo_box, container, row, text},
};
use log::debug;
use sqlx::{Sqlite, pool::PoolConnection};

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
    LoadPreview(SongPickerItem),
    PreviewLoaded(Song),
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::SongPicker(value)
    }
}

#[derive(Debug)]
/// Preview pro píseň
enum Preview {
    Empty,
    Loading(Handle),
    Loaded(Song),
}

impl Preview {
    pub fn new() -> Self {
        Self::Empty
    }

    /// Začne načítat dané preview.
    /// Vrátí Task, který reprezentuje načtení zdroje.
    /// - Pokud se Preview již načítá, původní task je ukončen (abort) a začne se načítat nový
    pub fn load(
        &mut self,
        fut: impl Future<Output = Result<Song>> + Send + 'static,
    ) -> Task<Result<Song>> {
        if let Preview::Loading(handle) = self {
            handle.abort();
        }

        let (task, handle) = Task::future(fut).abortable();

        *self = Preview::Loading(handle);

        task
    }

    /// Označí preview za načtené.
    pub fn loaded(&mut self, song: Song) {
        if let Preview::Loading(_) = self {
            *self = Preview::Loaded(song);
        } else {
            panic!("Zavoláno loaded() na Preview, které se nenačítalo");
        }
    }

    /// Vrátí Preview do původního (prázdného stavu)
    pub fn reset(&mut self) {
        *self = Preview::Empty
    }
}

#[derive(Debug)]
pub struct SongPicker {
    songs: Option<combo_box::State<SongPickerItem>>,
    playlist: PlaylistMetadata,
    preview: Preview,
}

impl SongPicker {
    pub fn new(playlist: PlaylistMetadata) -> Self {
        Self {
            songs: None,
            playlist,
            preview: Preview::Empty,
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
                    combo_box(combo_box_state, "Název písně", None, |item| {
                        Message::SongPicked(item.id)
                    })
                    .on_option_hovered(Message::LoadPreview),
                )
            })
            .unwrap_or(container(text("Načítám písně ...")));

        let preview = match &self.preview {
            Preview::Empty => container(Space::new(Length::Shrink, Length::Shrink)),
            Preview::Loading(_) => container(text("Načítám náhled")),
            Preview::Loaded(song) => song_preview(song),
        };

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
                column![
                    picker.align_bottom(Length::FillPortion(6)),
                    preview.height(Length::FillPortion(4))
                ]
                .spacing(10)
                .align_x(Alignment::Center)
                .width(Length::FillPortion(2)),
                container("").width(Length::FillPortion(1))
            ]
            .padding(10)
            .height(Length::Fill)
            .align_y(Alignment::Center),
        ))
        .explain(Color::BLACK)
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
        }
    }
}

fn song_preview(song: &Song) -> Container<'static, Message> {
    todo!()
}
