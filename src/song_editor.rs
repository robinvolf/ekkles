use ekkles_data::Song;
use iced::{
    Element, Length, Subscription, Task,
    alignment::{Horizontal, Vertical},
    widget::{button, column, container, row, space, text, text_editor, text_input},
};

use crate::Ekkles;

#[derive(Debug, Clone)]
pub enum Message {
    Save,
    Delete,
    Exit,
    SaveAsNameChanged(String),
    SaveAs,
    Editor(text_editor::Action),
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::SongEditor(value)
    }
}

#[derive(Debug)]
pub struct Editor {
    /// Právě editovaná píseň
    song: Song,
    /// Název nového playlistu pro "uložit jako"
    save_as_new_name: String,
    /// Chybný název nové písně (třeba taková už existuje)
    save_as_err_msg: Option<String>,
    /// Obsah textového editoru slov písně
    editor_content: text_editor::Content,
}

impl Editor {
    /// Vytvoří novou instanci editoru písně `song`. Předpokládá, že `song` je uložena v databázi.
    pub fn new(song: Song) -> Self {
        let song_text = song
            .parts
            .iter()
            .map(|(tag, lyrics)| format!("[{tag}]\n{lyrics}"))
            .collect::<Vec<String>>()
            .join("\n\n");

        Self {
            song,
            save_as_new_name: String::new(),
            save_as_err_msg: None,
            editor_content: text_editor::Content::with_text(&song_text),
        }
    }

    pub fn update(state: &mut Ekkles, msg: Message) -> Task<crate::Message> {
        todo!()
    }

    pub fn view(&self) -> Element<Message> {
        let left_panel = column![
            column![
                button("Uložit").on_press(Message::Save).width(Length::Fill),
                row![
                    text_input("Název nové písně", &self.save_as_new_name)
                        .on_input(Message::SaveAsNameChanged)
                        .on_submit(Message::SaveAs),
                    button("Uložit jako").on_press(Message::SaveAs)
                ]
                .width(Length::Fill),
                text(self.save_as_err_msg.as_ref().map_or("", String::as_str))
                    .style(text::danger)
                    .width(Length::Fill),
                button("Smazat píseň")
                    .style(button::danger)
                    .on_press(Message::Delete)
                    .width(Length::Fill),
            ]
            .width(Length::Fill)
            .padding(30)
            .spacing(10),
            container(button("Zpět").width(Length::Fill).on_press(Message::Exit))
                .padding(30)
                .align_y(Vertical::Bottom)
                .height(Length::Fill)
                .width(Length::Fill)
        ]
        .width(Length::FillPortion(1))
        .align_x(Horizontal::Center);

        let song_order = self
            .song
            .order
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ");

        let middle_panel = column![
            text_input("Název", &self.song.title),
            text("Slova písně"),
            text_editor(&self.editor_content).on_action(Message::Editor),
            row![text_input("Pořadí slok", &song_order)],
        ]
        .width(Length::FillPortion(2));

        let right_panel = space().width(Length::FillPortion(1)).height(Length::Fill);

        Into::<Element<Message>>::into(
            container(row![left_panel, middle_panel, right_panel]).padding(10),
        )
    }

    pub fn subscription(&self) -> Subscription<crate::Message> {
        Subscription::none()
    }
}
