use crate::{Screen, bible_picker, playlist_editor, presenter};
use crate::{song_picker, start_screen};
use iced::Task;
use log::{debug, trace, warn};

use crate::{Ekkles, Message};

impl Ekkles {
    pub fn update(&mut self, msg: Message) -> Task<Message> {
        trace!("Přišla zpráva: {:?}", msg);

        match (msg, &mut self.screen) {
            (Message::WindowOpened(_), Screen::PickEditor(_)) => Task::none(),
            (Message::WindowClosed(id), _) if id == self.main_window_id => {
                debug!("Hlavní okno zavřeno, ukončuji aplikaci");
                iced::exit()
            }
            (Message::WindowClosed(id), Screen::Presenter(presenter))
                if presenter
                    .get_window_id()
                    .is_some_and(|pres_id| id == pres_id) =>
            {
                debug!("Zavřeno okno prezentace");
                Task::done(crate::presenter::Message::PresentationWindowClosed.into())
            }
            (Message::PlaylistPicker(msg), Screen::PickEditor(_)) => {
                start_screen::update(self, msg)
            }
            (Message::PlaylistEditor(msg), Screen::EditPlaylist(_)) => {
                playlist_editor::PlaylistEditor::update(self, msg)
            }
            // (Message::SongPicker(msg), Screen::PickSong(_)) => {
            //     song_picker::SongPicker::update(self, msg)
            // }
            (Message::BiblePicker(msg), Screen::PickBible(_)) => {
                bible_picker::BiblePicker::update(self, msg)
            }
            (Message::Presenter(msg), Screen::Presenter(_)) => {
                presenter::Presenter::update(self, msg)
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
