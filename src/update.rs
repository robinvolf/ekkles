use crate::pick_playlist::Message as PpMessage;
use crate::{Screen, pick_playlist::PlaylistPickerItem};
use anyhow::Context;
use ekkles_data::playlist::{self, PlaylistMetadata};
use iced::Task;
use log::{debug, warn};

use crate::{Ekkles, Message, playlist_editor};

impl Ekkles {
    pub fn update(&mut self, msg: Message) -> Task<Message> {
        debug!("Přišla zpráva: {:?}", msg);

        match (msg, &mut self.screen) {
            (Message::WindowOpened(id), Screen::PickPlaylist(_icker)) => {
                if id == self.main_window_id {
                    debug!("Hlavní okno otevřeno, načítám playlisty z databáze");
                    // Vyrobíme future, kterou awaitneme v asynchronním bloku v Perform a ta nám vydá connection
                    let conn = self.db.acquire();
                    Task::perform(
                        async move {
                            let conn = conn.await.context("Nelze získat připojení k databázi")?;
                            playlist::get_available(conn).await
                        },
                        |res| match res {
                            Ok(pls) => Message::PlaylistPicker(PpMessage::PlaylistsLoaded(pls)),
                            Err(e) => Message::FatalErrorOccured(format!("{:?}", e)),
                        },
                    )
                } else {
                    todo!("Jiná okna nejsou implementována")
                }
            }
            (Message::WindowClosed(id), _) => {
                if id == self.main_window_id {
                    debug!("Hlavní okno zavřeno, ukončuji aplikaci");
                    iced::exit()
                } else {
                    todo!("Jiná okna nejsou implementována")
                }
            }
            (Message::PlaylistPicker(PpMessage::TopButtonSongs), Screen::PickPlaylist(_)) => {
                todo!("Ještě neumím editovat písně")
            }
            (Message::PlaylistPicker(PpMessage::TopButtonPlaylists), Screen::PickPlaylist(_)) => {
                debug!("Jsem v playlistu a klikám, abych se do něj znovu dostal, ignoruju");
                Task::none()
            }
            (
                Message::PlaylistPicker(PpMessage::PlaylistsLoaded(playlists)),
                Screen::PickPlaylist(picker),
            ) => {
                debug!("Načetly se playlisty");
                let options = playlists
                    .into_iter()
                    .map(|(id, name)| PlaylistPickerItem { id, name })
                    .collect();
                picker.playlists = Some(iced::widget::combo_box::State::new(options));
                Task::none()
            }
            (Message::PlaylistPicker(PpMessage::PickedPlaylist), Screen::PickPlaylist(_picker)) => {
                debug!("Byl vybrán playlist k otevření");
                todo!("Ještě neumím editovat playlisty")
            }
            (
                Message::PlaylistPicker(PpMessage::NewPlaylistNameChanged(input)),
                Screen::PickPlaylist(picker),
            ) => {
                picker.new_playlist_name = input;
                Task::none()
            }
            (
                Message::PlaylistPicker(PpMessage::CreateNewPlaylist),
                Screen::PickPlaylist(picker),
            ) => {
                let new_playlist = PlaylistMetadata::new(picker.new_playlist_name.trim());
                debug!("Vytvářím nový playlist \"{}\"", new_playlist.get_name());
                self.screen =
                    Screen::EditPlaylist(playlist_editor::PlaylistEditor::new(new_playlist));
                Task::none()
            }
            (Message::ShouldQuit, _) => {
                debug!("Ukončuji aplikaci");
                iced::exit()
            }
            (Message::FatalErrorOccured(e), _) => {
                self.screen = Screen::ErrorOccurred(e);
                Task::none()
            }
            (msg, screen) => {
                warn!(
                    "Neznámá kombinace zprávy a screen:\n{:#?}\n{:#?}",
                    msg, screen
                );
                Task::none()
            }
        }
    }
}
