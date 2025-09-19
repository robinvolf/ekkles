use iced::{
    Element, Length, Task,
    task::Handle,
    widget::{Space, button, container, row, text},
};
use log::warn;

use crate::Message;

pub mod playlist_item_styles;

#[derive(Debug, Clone, Copy)]
pub enum TopButtonsMessage {
    Playlists,
    Songs,
}

pub enum TopButtonsPickedSection {
    Songs,
    Playlists,
}

pub fn top_buttons(picked: TopButtonsPickedSection) -> Element<'static, TopButtonsMessage> {
    let (song_msg, playlist_msg) = match picked {
        TopButtonsPickedSection::Songs => (None, Some(TopButtonsMessage::Playlists)),
        TopButtonsPickedSection::Playlists => (Some(TopButtonsMessage::Songs), None),
    };
    row![
        button("Písně")
            .on_press_maybe(song_msg)
            .width(Length::FillPortion(1)),
        button("Playlisty")
            .on_press_maybe(playlist_msg)
            .width(Length::FillPortion(1))
    ]
    .width(Length::Fill)
    .into()
}

/// Preview pro generický typ T
#[derive(Debug)]
pub enum Preview<T> {
    Empty,
    Loading(Handle),
    Loaded(T),
}

impl<T> Preview<T> {
    pub fn new() -> Self {
        Self::Empty
    }

    /// Začne načítat dané preview.
    /// Vrátí Task, který reprezentuje načtení zdroje.
    /// - Pokud se Preview již načítá, původní task je ukončen (abort) a začne se načítat nový
    pub fn load<O: 'static>(&mut self, fut: impl Future<Output = O> + Send + 'static) -> Task<O> {
        if let Preview::Loading(handle) = self {
            handle.abort();
        }

        let (task, handle) = Task::future(fut).abortable();

        *self = Preview::Loading(handle);

        task
    }

    /// Označí preview za načtené.
    pub fn loaded(&mut self, previewed: T) {
        if let Preview::Loading(_) = self {
            *self = Preview::Loaded(previewed);
        } else {
            warn!(
                "Přišly data pro Preview, které se nenačítalo, může být vlivem zpoždění, ignoruju"
            );
        }
    }

    /// Vrátí Preview do původního (prázdného stavu)
    pub fn reset(&mut self) {
        *self = Preview::Empty
    }
}
