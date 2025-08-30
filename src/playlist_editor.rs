use std::sync::Arc;

use anyhow::Context;
use ekkles_data::{
    Song,
    playlist::{self, Playlist, PlaylistMetadata, PlaylistMetadataStatus},
};
use iced::{
    Background, Border, Color, Element, Length, Task, Theme,
    alignment::{Horizontal, Vertical},
    border::Radius,
    color,
    widget::{self, button, column, container, row, text, text_input},
};
use log::{debug, error, trace};
use tokio::sync::Mutex;

use crate::{
    Ekkles, Screen,
    bible_picker::BiblePicker,
    components::{TopButtonsMessage, TopButtonsPickedSection, top_buttons},
    pick_playlist::{self, PlaylistPicker},
    presenter::Presenter,
    song_picker::SongPicker,
};

const SONG_COLOR: Color = color!(0x02a2f6);
const PASSAGE_COLOR: Color = color!(0xfeaf4d);

#[derive(Debug, Clone)]
pub enum Message {
    TopButtonsPlaylist,
    TopButtonsSongs,
    LoadSongNameCache,
    SongNameCacheLoaded(Vec<(i64, String)>),
    SavePlaylist,
    PlaylistSavedSuccessfully,
    SavePlaylistAsClicked,
    NewPlaylistNameChanged(String),
    ValidateNewPlaylistName,
    NewPlaylistNameTaken,
    SavePlaylistAs,
    DeletePlaylist,
    SaveAndExit,
    ReturnToPlaylistPicker,
    LoadPresentation,
    StartPresentation(Presenter),
    AddBiblePassage,
    AddSong,
    SelectItem(usize),
    MoveItemUp(usize),
    MoveItemDown(usize),
    DeleteItem(usize),
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::PlaylistEditor(value)
    }
}

impl From<TopButtonsMessage> for Message {
    fn from(value: TopButtonsMessage) -> Self {
        match value {
            TopButtonsMessage::Playlists => Message::TopButtonsPlaylist,
            TopButtonsMessage::Songs => Message::TopButtonsSongs,
        }
    }
}

#[derive(Debug)]
pub struct PlaylistEditor {
    /// Editovaný playlist (potřebujeme ho zabalit do `Arc<Mutex<>>`, protože když jej ukládáme,
    /// mutujeme jeho stav a protože daný future předáme iced runtime, nemůže to být reference).
    playlist: Arc<Mutex<PlaylistMetadata>>,
    new_playlist_name: String,
    new_playlist_err_msg: String,
    song_name_cache: Option<Vec<(i64, String)>>,
    selected_index: Option<usize>,
}

impl PlaylistEditor {
    pub fn new(playlist: PlaylistMetadata) -> Self {
        Self {
            playlist: Arc::new(Mutex::new(playlist)),
            new_playlist_name: String::new(),
            new_playlist_err_msg: String::new(),
            song_name_cache: None,
            selected_index: None,
        }
    }

    pub fn view(&self) -> Element<Message> {
        let (playlist_status, playlist_name) = {
            // Tady blokuju čekáním na mutex v GUI kódu, ale contention tohoto mutexu
            // je prakticky nulová (zamykám ho jen při zápisu do DB, který je velice rychlý).
            let playlist = self.playlist.blocking_lock();
            let status = playlist.get_status();
            let name = playlist.get_name().to_string();
            (status, name)
        }; // V separátním scope, abychom tady dropli mutex

        let save_button_msg = match playlist_status {
            playlist::PlaylistMetadataStatus::Transient => Some(Message::SavePlaylist),
            playlist::PlaylistMetadataStatus::Clean(_) => None,
            playlist::PlaylistMetadataStatus::Dirty(_) => Some(Message::SavePlaylist),
        };

        // TODO: Chtělo by to modularizovat tyhle closury na určování stylu položek playlistu
        // Aktivní tlačítko je disabled, tudíž jeho status bude Disabled, naopak nevybraná
        // položka bude Active, to používám ve stylovací funkci, abych věděl, jestli
        // mám nastavit zvýraznění
        let song_style = |_: &Theme, _| button::Style {
            background: Some(Background::Color(SONG_COLOR)),
            border: Border {
                radius: Radius::new(0),
                ..Default::default()
            },
            ..Default::default()
        };

        let song_selected_style = |_: &Theme, _| button::Style {
            background: Some(Background::Color(SONG_COLOR)),
            border: Border {
                radius: Radius::new(0),
                color: Color::BLACK,
                width: 5.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let passage_style = |_: &Theme, _| button::Style {
            background: Some(Background::Color(PASSAGE_COLOR)),
            border: Border {
                radius: Radius::new(0),
                color: Color::BLACK,
                ..Default::default()
            },
            ..Default::default()
        };

        let passage_selected_style = |_: &Theme, _| button::Style {
            background: Some(Background::Color(PASSAGE_COLOR)),
            border: Border {
                radius: Radius::new(0),
                color: Color::BLACK,
                width: 5.0,
                ..Default::default()
            },
            ..Default::default()
        };

        // TODO: Vyřešit blokující lock v GUI kódu, problém je, že `playlist_items`
        // je iterátor a potřebuje, aby playlist byla validní reference, dokud nevrátíme
        // zkonstruované GUI
        let playlist = self.playlist.blocking_lock();

        let playlist_items = playlist
            .get_items()
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let msg = if self
                    .selected_index
                    .is_some_and(|selected| selected == index)
                {
                    None
                } else {
                    Some(Message::SelectItem(index))
                };

                match item {
                    playlist::PlaylistItemMetadata::BiblePassage { from, to, .. } => {
                        button(text(format!("Pasáž {} - {}", from, to)))
                            .style(if msg.is_none() {
                                song_selected_style
                            } else {
                                song_style
                            })
                            .on_press_maybe(msg)
                            .width(Length::Fill)
                            .into()
                    }
                    playlist::PlaylistItemMetadata::Song(sought_id) => button(text(format!(
                        "Píseň {}",
                        self.song_name_cache
                            .as_ref()
                            .map(|cache| cache
                                .iter()
                                .find(|(id, _)| id == sought_id)
                                .unwrap()
                                .1
                                .as_str())
                            .unwrap_or("...")
                    )))
                    .style(if msg.is_none() {
                        passage_selected_style
                    } else {
                        passage_style
                    })
                    .on_press_maybe(msg)
                    .width(Length::Fill)
                    .into(),
                }
            });

        let item_manipulation = match self.selected_index {
            Some(index) => {
                column![
                    button("Posunout nahoru")
                        .on_press_maybe(if index == 0 {
                            None
                        } else {
                            Some(Message::MoveItemUp(index))
                        })
                        .width(Length::Fill),
                    button("Posunout dolů")
                        // len() - 1 je v pořádku, nikdy nepodteče, tento kód se provede pouze
                        // s vybranou položkou, nelze mít vybranou položku na prázdném seznamu
                        .on_press_maybe(if index == playlist.get_items().len() - 1 {
                            None
                        } else {
                            Some(Message::MoveItemDown(index))
                        })
                        .width(Length::Fill),
                    button("Smazat položku")
                        .on_press(Message::DeleteItem(index))
                        .style(button::danger)
                        .width(Length::Fill),
                ]
            }
            None => column([]),
        };

        Into::<Element<Message>>::into(column![
            top_buttons(TopButtonsPickedSection::Playlists).map(|msg| msg.into()),
            container(row![
                column![
                    column![
                        text(format!("Edituješ playlist \"{}\"", playlist_name)),
                        button("Uložit")
                            .on_press_maybe(save_button_msg)
                            .width(Length::Fill),
                        row![
                            text_input("Název nového playlistu", &self.new_playlist_name)
                                .on_input(Message::NewPlaylistNameChanged)
                                .on_submit(Message::SavePlaylistAsClicked),
                            button("Uložit jako").on_press(Message::SavePlaylistAsClicked)
                        ]
                        .width(Length::Fill),
                        text(&self.new_playlist_err_msg)
                            .style(text::danger)
                            .width(Length::Fill),
                        button("Smazat playlist")
                            .style(button::danger)
                            .on_press(Message::DeletePlaylist)
                            .width(Length::Fill),
                        button("Přidat píseň")
                            .on_press(Message::AddSong)
                            .width(Length::Fill),
                        button("Přidat verše")
                            .on_press(Message::AddBiblePassage)
                            .width(Length::Fill),
                        button("Prezentovat")
                            .on_press(Message::LoadPresentation)
                            .width(Length::Fill)
                    ]
                    .width(Length::Fill)
                    .padding(30)
                    .spacing(10),
                    container(
                        button("Zpět")
                            .width(Length::Fill)
                            .on_press(Message::SaveAndExit)
                    )
                    .padding(30)
                    .align_y(Vertical::Bottom)
                    .height(Length::Fill)
                    .width(Length::Fill)
                ]
                .width(Length::FillPortion(1))
                .align_x(Horizontal::Center),
                column(playlist_items)
                    .padding(30)
                    .spacing(5)
                    .width(Length::FillPortion(2)),
                if self.selected_index.is_some() {
                    item_manipulation
                } else {
                    column([])
                }
                .width(Length::FillPortion(1))
                .padding(30)
                .spacing(10),
            ])
            .padding(10)
            .center_x(Length::FillPortion(1))
        ])
        // .explain(Color::BLACK)
    }

    /// Update funkce pro editor. Pokud je tato funkce zavolána nad jinou obrazovkou
    /// než [`Screen::EditPlaylist`], zpanikaří.
    pub fn update(state: &mut Ekkles, msg: Message) -> Task<crate::Message> {
        let editor = match &mut state.screen {
            Screen::EditPlaylist(editor) => editor,
            screen => panic!("Update pro Editor zavolán, nad obrazovkou {:#?}", screen),
        };

        match msg {
            Message::SavePlaylist => {
                debug!("Ukládám playlist");
                let conn = state.db.acquire();
                let playlist = editor.playlist.clone();
                Task::perform(
                    async move {
                        let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                        let mut playlist = playlist.lock().await;
                        playlist.save(&mut conn).await
                    },
                    |res| match res {
                        Ok(_) => Message::PlaylistSavedSuccessfully.into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::SavePlaylistAs => {
                debug!(
                    "Ukládám playlist pod novým názvem: \"{}\"",
                    &editor.new_playlist_name
                );
                let conn = state.db.acquire();
                let new_playlist_name = editor.new_playlist_name.clone();
                let playlist = editor.playlist.clone();
                Task::perform(
                    async move {
                        let mut playlist = playlist.lock().await;

                        *playlist = playlist::PlaylistMetadata::from_other(
                            &new_playlist_name,
                            &mut playlist,
                        );

                        let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                        playlist.save(&mut conn).await
                    },
                    |res| match res {
                        Ok(_) => Message::PlaylistSavedSuccessfully.into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::LoadPresentation => {
                debug!("Načítám prezentaci");
                let conn = state.db.acquire();
                let playlist = editor.playlist.clone();
                Task::perform(
                    async move {
                        let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                        let mut playlist = playlist.lock().await;
                        playlist
                            .save(&mut conn)
                            .await
                            .context("Nelze uložit playlist")?;

                        let id = if let PlaylistMetadataStatus::Clean(id) = playlist.get_status() {
                            id
                        } else {
                            unreachable!() // Právě jsme uložili playlist, musí být ve stavu Clean
                        };

                        Presenter::try_new(id, &mut conn).await
                    },
                    |res| match res {
                        Ok(presenter) => Message::StartPresentation(presenter).into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }

            Message::StartPresentation(presenter) => {
                debug!("Přecházím na prezentační obrazovku");
                state.screen = Screen::Presenter(presenter);
                Task::done(crate::presenter::Message::OpenPresentationWindow.into())
            }
            Message::AddBiblePassage => {
                debug!("Přecházím na výběr playlistu");
                let playlist = editor.playlist.blocking_lock().clone();
                state.screen = Screen::PickBible(BiblePicker::new(playlist));
                Task::done(crate::Message::BiblePicker(
                    crate::bible_picker::Message::LoadTranslations,
                ))
            }
            Message::AddSong => {
                debug!("Přecházím na výběr písně");
                let playlist = editor.playlist.blocking_lock().clone();
                state.screen = Screen::PickSong(SongPicker::new(playlist));
                Task::done(crate::Message::SongPicker(
                    crate::song_picker::Message::LoadSongs,
                ))
            }
            Message::PlaylistSavedSuccessfully => {
                debug!("Playlist byl úspéšně uložen");
                editor.new_playlist_name.clear();
                editor.new_playlist_err_msg.clear();
                Task::none()
            }
            Message::SavePlaylistAsClicked => Task::done(Message::ValidateNewPlaylistName.into()),
            Message::ValidateNewPlaylistName => {
                debug!("Zjišťuji, jestli se v databázi nachází playlist s daným názvem");
                let conn = state.db.acquire();
                let name = editor.new_playlist_name.clone();
                Task::perform(
                    async move {
                        let conn = conn.await.context("Nelze získat připojení k databázi")?;
                        playlist::is_name_available(conn, &name).await
                    },
                    |res| match res {
                        Ok(available) => {
                            if available {
                                Message::SavePlaylistAs.into()
                            } else {
                                Message::NewPlaylistNameTaken.into()
                            }
                        }
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::NewPlaylistNameTaken => {
                debug!("Nastavuji chybovou hlášku, aby uživatel změnil název nového playlistu");
                editor.new_playlist_err_msg =
                    String::from("Playlist s daným názvem již existuje, vyber jiný");
                Task::none()
            }
            Message::TopButtonsPlaylist => todo!(),
            Message::TopButtonsSongs => {
                todo!("Ještě neumím editovat písně")
            }
            Message::NewPlaylistNameChanged(input) => {
                trace!("Změnil se nový název playlistu: {input}");
                editor.new_playlist_name = input;
                Task::none()
            }
            Message::DeletePlaylist => {
                {
                    debug!(
                        "Mažu playlist \"{}\"",
                        editor.playlist.blocking_lock().get_name()
                    )
                }

                let conn = state.db.acquire();
                let playlist = editor.playlist.clone();
                Task::perform(
                    async move {
                        let mut conn = conn.await?;
                        let mut playlist = playlist.lock().await;
                        playlist.delete(&mut conn).await
                    },
                    |res| match res {
                        Ok(_) => Message::ReturnToPlaylistPicker.into(),
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::ReturnToPlaylistPicker => {
                state.screen = Screen::PickPlaylist(PlaylistPicker::default());
                Task::done(crate::Message::PlaylistPicker(
                    pick_playlist::Message::LoadPlaylists,
                ))
            }
            Message::SaveAndExit => {
                let playlist_status = { editor.playlist.blocking_lock().get_status() };

                match playlist_status {
                    PlaylistMetadataStatus::Transient | PlaylistMetadataStatus::Dirty(_) => {
                        debug!("Ukládám playlist a vracím se k výběru playlistů");
                        let conn = state.db.acquire();
                        let playlist = editor.playlist.clone();
                        Task::perform(
                            async move {
                                let mut conn =
                                    conn.await.context("Nelze získat připojení k databázi")?;
                                let mut playlist = playlist.lock().await;
                                playlist.save(&mut conn).await
                            },
                            |res| res,
                        )
                        .then(|res| {
                            debug!("Playlist uložen, vracím se na výběr playlistů");
                            match res {
                                Ok(_) => Task::batch([
                                    Task::done(Message::ReturnToPlaylistPicker.into()),
                                    Task::done(crate::pick_playlist::Message::LoadPlaylists.into()),
                                ]),
                                Err(e) => Task::done(crate::Message::FatalErrorOccured(format!(
                                    "{:?}",
                                    e
                                ))),
                            }
                        })
                    }
                    PlaylistMetadataStatus::Clean(_) => {
                        debug!("Playlist nepotřebuje uložit, vracím se rovnou na výběr playlistů");
                        Task::batch([
                            Task::done(Message::ReturnToPlaylistPicker.into()),
                            Task::done(crate::pick_playlist::Message::LoadPlaylists.into()),
                        ])
                    }
                }
            }
            Message::LoadSongNameCache => {
                debug!("Načítám cache názvů písní");
                let conn = state.db.acquire();
                Task::perform(
                    async move {
                        let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                        Song::get_available_from_db(&mut conn).await
                    },
                    |res| res,
                )
                .then(|res| match res {
                    Ok(cache) => Task::done(Message::SongNameCacheLoaded(cache).into()),
                    Err(e) => Task::done(crate::Message::FatalErrorOccured(format!("{:?}", e))),
                })
            }
            Message::SongNameCacheLoaded(items) => {
                debug!("Načtena cache názvů písní");
                editor.song_name_cache = Some(items);
                Task::none()
            }
            Message::SelectItem(index) => {
                debug!("Vybrána položka playlistu {index}");
                editor.selected_index = Some(index);
                Task::none()
            }
            Message::MoveItemUp(index) => {
                debug!("Posunuji položku na indexu {index} na {}", index - 1);
                *editor
                    .selected_index
                    .as_mut()
                    .expect("Při posunování vybrané položka musí být položka vybrána") -= 1;
                let playlist = editor.playlist.clone();
                Task::future(async move {
                    let mut playlist = playlist.lock().await;
                    playlist
                        .swap_items(index, index - 1)
                        .expect("Nelze posunout položku nahoru");
                })
                .discard()
            }
            Message::MoveItemDown(index) => {
                debug!("Posunuji položku na indexu {index} na {}", index + 1);
                *editor
                    .selected_index
                    .as_mut()
                    .expect("Při posunování vybrané položka musí být položka vybrána") += 1;

                let playlist = editor.playlist.clone();
                Task::future(async move {
                    let mut playlist = playlist.lock().await;
                    playlist
                        .swap_items(index, index + 1)
                        .expect("Nelze posunout položku dolů");
                })
                .discard()
            }
            Message::DeleteItem(index) => {
                debug!("Mažu položku s indexem {index}");
                editor.selected_index = None;
                let playlist = editor.playlist.clone();
                Task::future(async move {
                    let mut playlist = playlist.lock().await;
                    playlist.delete_item(index).expect("Nelze smazat položku");
                })
                .discard()
            }
        }
    }
}
