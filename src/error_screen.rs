use iced::{
    Element, Length,
    alignment::Horizontal,
    widget::{button, column, container, text},
};

use crate::Message;

pub fn view(error: &str) -> Element<'static, Message> {
    container(
        column!(
            text(format!("Došlo k chybě: {}", error)),
            button("Ukončit aplikaci").on_press(Message::ShouldQuit)
        )
        .spacing(20)
        .align_x(Horizontal::Center),
    )
    .center(Length::Fill)
    .into()
}
