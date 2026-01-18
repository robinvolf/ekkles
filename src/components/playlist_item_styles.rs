use ekkles_data::playlist::{PlaylistItem, PlaylistItemMetadata};
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

/// Vrátí styl tlačítka, podle toho, co je to za položku a jestli je vybrána
pub fn playlist_item_button_style(
    item: &PlaylistItemMetadata,
    selected: bool,
) -> fn(&Theme, button::Status) -> button::Style {
    const SONG: Color = color!(0x7bccf6);
    const PASSAGE: Color = color!(0xfec57f);
    const SELECTED_SCALER: f32 = 0.7;
    const SONG_SELECTED: Color = {
        let mut c = SONG;
        c.r *= SELECTED_SCALER;
        c.g *= SELECTED_SCALER;
        c.b *= SELECTED_SCALER;
        c
    };
    const PASSAGE_SELECTED: Color = {
        let mut c = PASSAGE;
        c.r *= SELECTED_SCALER;
        c.g *= SELECTED_SCALER;
        c.b *= SELECTED_SCALER;
        c
    };

    match (item, selected) {
        (PlaylistItemMetadata::BiblePassage { .. }, true) => {
            |_theme: &Theme, _status| button::Style {
                background: Some(Background::Color(PASSAGE_SELECTED)),
                ..Default::default()
            }
        }
        (PlaylistItemMetadata::BiblePassage { .. }, false) => {
            |_theme: &Theme, _status| button::Style {
                background: Some(Background::Color(PASSAGE)),
                ..Default::default()
            }
        }
        (PlaylistItemMetadata::Song(_), true) => |_theme: &Theme, _status| button::Style {
            background: Some(Background::Color(SONG_SELECTED)),
            ..Default::default()
        },
        (PlaylistItemMetadata::Song(_), false) => |_theme: &Theme, _status| button::Style {
            background: Some(Background::Color(SONG)),
            ..Default::default()
        },
    }
}

// TODO: Unifikovat s funkcí pro metadata
pub fn playlist_item_button_style2(
    item: &PlaylistItem,
    selected: bool,
) -> fn(&Theme, button::Status) -> button::Style {
    const SONG: Color = color!(0x7bccf6);
    const PASSAGE: Color = color!(0xfec57f);
    const SELECTED_SCALER: f32 = 0.7;
    const SONG_SELECTED: Color = {
        let mut c = SONG;
        c.r *= SELECTED_SCALER;
        c.g *= SELECTED_SCALER;
        c.b *= SELECTED_SCALER;
        c
    };
    const PASSAGE_SELECTED: Color = {
        let mut c = PASSAGE;
        c.r *= SELECTED_SCALER;
        c.g *= SELECTED_SCALER;
        c.b *= SELECTED_SCALER;
        c
    };

    match (item, selected) {
        (PlaylistItem::BiblePassage { .. }, true) => |_theme: &Theme, _status| button::Style {
            background: Some(Background::Color(PASSAGE_SELECTED)),
            ..Default::default()
        },
        (PlaylistItem::BiblePassage { .. }, false) => |_theme: &Theme, _status| button::Style {
            background: Some(Background::Color(PASSAGE)),
            ..Default::default()
        },
        (PlaylistItem::Song(_), true) => |_theme: &Theme, _status| button::Style {
            background: Some(Background::Color(SONG_SELECTED)),
            ..Default::default()
        },
        (PlaylistItem::Song(_), false) => |_theme: &Theme, _status| button::Style {
            background: Some(Background::Color(SONG)),
            ..Default::default()
        },
    }
}
