use crate::pick_playlist::{self, Message as PpMessage};
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
            (Message::PlaylistPicker(msg), Screen::PickPlaylist(_)) => {
                pick_playlist::update(self, msg)
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
