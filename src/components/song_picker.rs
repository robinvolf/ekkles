//! # Komponenta pro výběr písní
//! Obsahuje combobox s jednotlivými písněmi a náhled právě označené písně.

use crate::components::{LazyLoadable, LazyLoadableState, PickerItem};
use ekkles_data::Song;
use iced::{
    Alignment, Element, Length, Task,
    widget::{Container, button, column, combo_box, container, row, rule, scrollable, space, text},
};
use log::debug;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub enum Message {
    LoadSongs,
    SongsLoaded(Vec<PickerItem>, u32),
    SelectSong(PickerItem),
    PreviewChanged(PickerItem),
    LoadPreview(PickerItem),
    PreviewLoaded(Song, u32),
    Return,
    ReturnSelected(PickerItem),
    FatalError(String),
}

#[derive(Debug)]
pub struct SongPicker {
    songs: LazyLoadable<combo_box::State<PickerItem>, Message>,
    selected: Option<PickerItem>,
    preview: Option<LazyLoadable<Song, Message>>,
}

impl SongPicker {
    pub fn new() -> Self {
        Self {
            songs: LazyLoadable::new(Message::LoadSongs),
            selected: None,
            preview: None,
        }
    }

    pub fn songs(&self) -> &LazyLoadable<combo_box::State<PickerItem>, Message> {
        &self.songs
    }

    pub fn view(&self) -> Element<Message> {
        let picker = match &self.songs.state() {
            LazyLoadableState::Cold | LazyLoadableState::Loading { .. } => {
                self.songs.view_not_loaded()
            }
            LazyLoadableState::Loaded(combo_box_state) => container(
                combo_box(
                    combo_box_state,
                    "Název písně",
                    self.selected.as_ref(),
                    Message::SelectSong,
                )
                .on_option_hovered(Message::PreviewChanged),
            ),
        };

        let preview = match &self.preview {
            Some(l) => match l.state() {
                LazyLoadableState::Cold | LazyLoadableState::Loading { .. } => l.view_not_loaded(),
                LazyLoadableState::Loaded(s) => song_preview(s),
            },
            None => container(space()),
        };

        Into::<Element<Message>>::into(container(
            column![
                row![
                    picker.align_top(Length::Fill).center_x(Length::Fill),
                    preview.center(Length::Fill)
                ]
                .spacing(10),
                row![
                    button("Zpět")
                        .on_press(Message::Return)
                        .width(Length::Fill)
                        .height(Length::Shrink),
                    button("Potvrdit")
                        .width(Length::Fill)
                        .height(Length::Shrink)
                        .on_press_maybe(
                            self.selected
                                .as_ref()
                                .map(|s| Message::ReturnSelected(s.clone()))
                        )
                ]
                .spacing(10)
            ]
            .spacing(10),
        ))
    }

    pub fn update(&mut self, db: &SqlitePool, message: Message) -> Task<Message> {
        match message {
            Message::LoadSongs => {
                debug!("Načítám seznam písní");
                let conn = db.acquire();
                let (task, handle) = Task::abortable(Task::future(async {
                    let mut conn = conn.await?;
                    Song::get_available_from_db(&mut conn).await.map(|vec| {
                        vec.into_iter()
                            .map(|(id, name)| PickerItem::new(id, name))
                            .collect()
                    })
                }));
                let task_id = self.songs.start_loading(handle);
                let task = Task::map(task, move |res| match res {
                    Ok(songs) => Message::SongsLoaded(songs, task_id).into(),
                    Err(e) => Message::FatalError(format!("{:?}", e)),
                });
                task
            }
            Message::SongsLoaded(song_picker_items, task_id) => {
                debug!("Písně načteny: {:#?}", &song_picker_items);
                self.songs
                    .finish_loading(combo_box::State::new(song_picker_items), task_id);

                Task::none()
            }
            Message::PreviewChanged(item) => {
                debug!("Preview nastaveno pro píseň {}", item.name);

                self.preview.as_mut().map(|p| p.cancel_loading_opt());
                self.preview = Some(LazyLoadable::new(Message::LoadPreview(item)));

                Task::none()
            }
            Message::LoadPreview(item) => {
                debug!("Načítám preview pro píseň {}", item.name);
                let conn = db.acquire();
                let (task, handle) = Task::abortable(Task::future(async move {
                    let mut conn = conn.await?;
                    Song::load_from_db(item.id, &mut conn).await
                }));
                let task_id = self
                    .preview
                    .as_mut()
                    .expect("Při zavolání LoadPreview již musí být preview Some()")
                    .start_loading(handle);
                let task = Task::map(task, move |res| match res {
                    Ok(song) => Message::PreviewLoaded(song, task_id).into(),
                    Err(e) => Message::FatalError(format!("{:?}", e)),
                });
                task
            }
            Message::PreviewLoaded(song, task_id) => {
                debug!("Načetlo se previw pro píseň {}", song.title);
                self.preview
                    .as_mut()
                    .expect("Při zavolání LoadPreview již musí být preview Some()")
                    .finish_loading(song, task_id);
                Task::none()
            }
            m @ Message::ReturnSelected(_) | m @ Message::Return | m @ Message::FatalError(_) => {
                panic!(
                    "Zpráva {:?} byla přeposlána až do update komponenty SongPicker, toto by se nemělo stát. Měl bys na tuto zprávu reagovat tam, kde komponentu používáš a zavřít komponentu",
                    m
                );
            }
            Message::SelectSong(item) => {
                self.selected = Some(item);
                Task::none()
            }
        }
    }
}

/// Vytvoří preview písně
fn song_preview(song: &Song) -> Container<Message> {
    let lyrics: String = song
        .order
        .iter()
        .map(|part| format!("[{}]\n{}\n", part, song.parts.get(part).unwrap()))
        .collect();

    container(column![
        text(song.title.clone()).center().width(Length::Fill),
        rule::horizontal(2),
        container(
            scrollable(text(lyrics).align_x(Alignment::Center).width(Length::Fill))
                .width(Length::Fill)
        ),
    ])
}
