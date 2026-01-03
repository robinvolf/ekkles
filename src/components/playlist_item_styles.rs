use iced::{Background, Border, Color, Theme, border::Radius, color, widget::button};

const SONG_COLOR: Color = color!(0x7bccf6);
const PASSAGE_COLOR: Color = color!(0xfec57f);
const SELECTED_COLOR: Color = color!(0x89fe7f);

pub fn song(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(SONG_COLOR)),
        border: Border {
            radius: Radius::new(0),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn song_selected(theme: &Theme, status: button::Status) -> button::Style {
    let style = song(theme, status);
    style.with_background(Background::Color(SELECTED_COLOR))
}

pub fn passage(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(PASSAGE_COLOR)),
        border: Border {
            radius: Radius::new(0),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn passage_selected(theme: &Theme, status: button::Status) -> button::Style {
    let style = passage(theme, status);
    style.with_background(Background::Color(SELECTED_COLOR))
}
