use iced::{Background, Border, Color, Theme, border::Radius, color, widget::button};

const SONG_COLOR: Color = color!(0x02a2f6);
const PASSAGE_COLOR: Color = color!(0xfeaf4d);

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
    let mut style = song(theme, status);
    style.border.width = 5.0;
    style.border.color = Color::BLACK;
    style
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
    let mut style = passage(theme, status);
    style.border.width = 5.0;
    style.border.color = Color::BLACK;
    style
}
