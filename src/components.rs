use iced::{
    Element, Length,
    widget::{button, row},
};

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
