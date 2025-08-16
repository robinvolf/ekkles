use iced::{
    Element, Length,
    widget::{button, row},
};

#[derive(Debug, Clone, Copy)]
pub enum TopButtonsMessage {
    Playlists,
    Songs,
}

pub fn top_buttons() -> Element<'static, TopButtonsMessage> {
    row![
        button("Písně")
            .on_press(TopButtonsMessage::Songs)
            .width(Length::FillPortion(1)),
        button("Playlisty")
            .on_press(TopButtonsMessage::Playlists)
            .width(Length::FillPortion(1))
    ]
    .width(Length::Fill)
    .into()
}
