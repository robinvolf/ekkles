use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::RandomState;
use std::iter::once;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use ekkles_data::Song;
use ekkles_data::song_xml::parse_lyrics;
use iced::keyboard::Key;
use iced::keyboard::Modifiers;
use iced::keyboard::key;
use iced::widget::rule;
use iced::widget::toggler;
use iced::{
    Element, Length, Subscription, Task,
    alignment::{Horizontal, Vertical},
    widget::{button, column, container, row, text, text_editor, text_input},
};
use log::{debug, trace};

use crate::components::shortcuts::KeyboardShortcut;
use crate::{Ekkles, Screen, start_screen::StartScreen};

#[derive(Debug, Clone)]
pub enum Message {
    Save,
    Delete,
    Exit,
    SaveAsNameChanged(String),
    SaveAs,
    SavedUnderNewId(i64),
    Editor(text_editor::Action),
    ChangedOrderInputMode(OrderInput),
    ChangedOrderManually(String),
    AuthorNameChanged(String),
}

#[derive(Debug, Clone, Copy, Hash)]
enum KeyboardShortcutMessage {
    Exit,
}

impl From<KeyboardShortcutMessage> for Message {
    fn from(value: KeyboardShortcutMessage) -> Self {
        match value {
            KeyboardShortcutMessage::Exit => Message::Exit,
        }
    }
}

/// Jestli se pořadí slok odvodí automaticky z textu
/// nebo ho uživatel bude manuálně upravovat
#[derive(Debug, Clone, Copy)]
pub enum OrderInput {
    Automatic,
    Manual,
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::SongEditor(value)
    }
}

/// Maximální počet znaků sloky, které se zapíší do náhledu
const PARTS_PREVIEW_MAX_CHARS: usize = 20;

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
    /// Způsob, jak se bude zadávat pořadí slok
    order_input: OrderInput,
    /// Manuální vstup pořadí slok, držíme si ho separátně mimo `song`,
    /// protože může být nevalidní
    order_input_manual_buffer: String,
    /// Chybová hláška, která se zobrazí, když je manuální vstup
    /// pořadí slok chybný
    order_input_manual_error: Option<String>,
    /// Klávesové zkratky
    shortcuts: [KeyboardShortcut<KeyboardShortcutMessage>; 1],
}

impl Editor {
    /// Vytvoří novou instanci editoru písně `song`. Předpokládá, že `song` je uložena v databázi.
    pub fn new(song: Song) -> Self {
        let song_text = song
            .order
            .iter()
            .map(|tag| {
                let lyrics = &song.parts[tag];
                format!("[{tag}]\n{lyrics}")
            })
            .collect::<Vec<String>>()
            .join("\n\n");

        Self {
            song,
            save_as_new_name: String::new(),
            save_as_err_msg: None,
            editor_content: text_editor::Content::with_text(&song_text),
            order_input: OrderInput::Automatic,
            order_input_manual_buffer: String::new(),
            order_input_manual_error: None,
            shortcuts: [KeyboardShortcut::new(
                Key::Named(key::Named::Escape),
                Modifiers::empty(),
                KeyboardShortcutMessage::Exit,
                "Zpět na výběr písní",
            )],
        }
    }

    pub fn update(state: &mut Ekkles, msg: Message) -> Task<crate::Message> {
        let editor = if let Screen::SongEditor(editor) = &mut state.screen {
            editor
        } else {
            panic!("Update pro SongEditor zavolána na jinou obrazovku");
        };

        match msg {
            Message::Save => {
                debug!("Ukládám píseň {:?}", editor.song);
                let song_copy = editor.song.clone();
                let conn = state.db.acquire();

                Task::future(async move {
                    let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                    song_copy.update(&mut conn).await
                })
                .then(|res| match res {
                    Ok(_) => Task::none(),
                    Err(e) => Task::done(crate::Message::FatalErrorOccured(e.to_string())),
                })
            }
            Message::Delete => {
                debug!("Mažu píseň {:?}", editor.song);
                let conn = state.db.acquire();
                let id = editor.song.id;

                Task::future(async move {
                    let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                    Song::delete_from_db(id, &mut conn).await
                })
                .then(|res| match res {
                    Ok(_) => Task::done(Message::Exit.into()),
                    Err(e) => Task::done(crate::Message::FatalErrorOccured(e.to_string())),
                })
            }
            Message::Exit => {
                debug!("Vracím se na startovací obrazovku");

                state.screen = Screen::StartScreen(StartScreen::new());
                Task::none()
            }
            Message::SaveAsNameChanged(name) => {
                trace!("Nastavuji název pro uložení písně pod novým jménem na {name}");

                editor.save_as_new_name = name;
                Task::none()
            }
            Message::SaveAs => {
                debug!(
                    "Ukládám píseň {:?} pod novým názvem {}",
                    editor.song, editor.save_as_new_name
                );

                let song_copy = Song {
                    title: editor.save_as_new_name.clone(),
                    ..editor.song.clone()
                };
                let conn = state.db.acquire();

                Task::future(async move {
                    let mut conn = conn.await.context("Nelze získat připojení k databázi")?;
                    song_copy.save_new(&mut conn).await
                })
                .then(|res| match res {
                    Ok(id) => Task::done(Message::SavedUnderNewId(id).into()),
                    Err(e) => Task::done(crate::Message::FatalErrorOccured(e.to_string())),
                })
            }
            Message::Editor(action) => {
                trace!("Provádím akci textového editoru {:?}", action);
                editor.editor_content.perform(action);

                let lyrics_iter = parse_lyrics(&editor.editor_content.text()).into_iter();
                let lyrics = lyrics_iter.clone().collect::<HashMap<_, _>>();
                let order = lyrics_iter.map(|(tag, _lyrics)| tag).collect::<Vec<_>>();
                trace!(
                    "Update interního songu podle aktuálního obsahu textového editoru, slova: {:?}, pořadí: {:?}",
                    lyrics, order
                );

                editor.song.parts = lyrics;
                editor.song.order = order;

                Task::none()
            }
            Message::SavedUnderNewId(id) => {
                debug!("Updatuji ID písně, pod kterým byla nově uložena");
                editor.song.id = id;
                Task::none()
            }
            Message::ChangedOrderInputMode(order_input) => {
                debug!("Nastavuji metodu vstupu na {:?}", order_input);
                editor.order_input = order_input;

                // Pokud jsme právě přepli na manuální vstup, zkopírujeme
                // si kanonicky správné pořadí do bufferu pro manuální vstup
                if let OrderInput::Manual = order_input {
                    editor.order_input_manual_buffer = editor.song.order.join(" ");
                }

                Task::none()
            }
            Message::AuthorNameChanged(name) => {
                editor.song.author = if name.is_empty() { None } else { Some(name) };
                trace!("Název autora se změnil na {:?}", editor.song.author);
                Task::none()
            }
            Message::ChangedOrderManually(o) => {
                let parsed_order =
                    parse_song_order(&o, HashSet::from_iter(editor.song.order.iter().cloned()));
                editor.order_input_manual_buffer = o;

                match parsed_order {
                    Ok(parsed) => {
                        trace!(
                            "Manuální pořadí úspěšně zparsováno, nastavuji pro píseň na {:?} a vyčišťuji chybovou hlášku",
                            &parsed
                        );
                        editor.song.order = parsed;
                        editor.order_input_manual_error = None;
                    }
                    Err(e) => {
                        let e_str = e.to_string();
                        trace!("Parsování manuálního pořadí selhalo, nastavuji chybovou hlášku");
                        editor.order_input_manual_error = Some(e_str);
                    }
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
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

        let song_order_selection = {
            let toggle_msg = move |b: bool| match b {
                true => Message::ChangedOrderInputMode(OrderInput::Automatic),
                false => Message::ChangedOrderInputMode(OrderInput::Manual),
            };
            let (label, toggle_state, order_content) = match &self.order_input {
                OrderInput::Automatic => ("Automaticky", true, Element::from(text(song_order))),
                OrderInput::Manual => (
                    "Manuálně",
                    false,
                    Element::from(
                        text_input("", &self.order_input_manual_buffer)
                            .on_input(Message::ChangedOrderManually),
                    ),
                ),
            };

            column![
                row![
                    order_content,
                    container(toggler(toggle_state).on_toggle(toggle_msg).label(label))
                        .align_right(Length::Fill),
                ]
                .spacing(5),
                text(
                    self.order_input_manual_error
                        .as_ref()
                        .map_or("", String::as_str)
                )
                .style(text::danger)
            ]
        };

        let middle_panel = column![
            text!("Píseň: {}", &self.song.title),
            rule::horizontal(1),
            text("Slova písně"),
            text_editor(&self.editor_content)
                .on_action(Message::Editor)
                .max_height(400),
            rule::horizontal(1),
            text("Pořadí slok"),
            song_order_selection,
            rule::horizontal(1),
            text("Autor písně"),
            text_input(
                "Autor",
                self.song.author.as_ref().map_or("", String::as_str)
            )
            .on_input(Message::AuthorNameChanged)
        ]
        .spacing(10)
        .width(Length::FillPortion(2));

        let parsed_preview = self.song.order.iter().map(|tag| {
            let part_content: String = self.song.parts[tag]
                .chars()
                .map(|c| if c == '\n' { ' ' } else { c })
                .take(PARTS_PREVIEW_MAX_CHARS)
                .chain(once('…'))
                .collect::<String>();
            text!("{}: {}", tag, part_content)
                .wrapping(text::Wrapping::None)
                .height(Length::Fixed(20.0))
                .into()
        });

        let right_panel = column![
            text("Náhled slok"),
            column(parsed_preview),
            container(KeyboardShortcut::view(&self.shortcuts)).align_bottom(Length::Fill)
        ]
        .spacing(10)
        .padding(30)
        .width(Length::FillPortion(1))
        .height(Length::Fill);

        Into::<Element<Message>>::into(
            container(row![left_panel, middle_panel, right_panel]).padding(10),
        )
    }

    pub fn subscription(&self) -> Subscription<crate::Message> {
        KeyboardShortcut::subscription(self.shortcuts.clone())
            .map(Message::from)
            .map(crate::Message::from)
    }
}

/// Pokusí se zparsovat `raw_order` pomocí `parsed_order_tags`. Zajistí, že:
/// - Každá sloka je alespoň jedenkrát přítomná
/// - Neexistuje sloka, která by nebyla v `parsed_order_tags`
fn parse_song_order(raw_order: &str, parsed_order_tags: HashSet<String>) -> Result<Vec<String>> {
    let order = raw_order
        .split_whitespace()
        .map(String::from)
        .collect::<Vec<String>>();

    let order_as_set = HashSet::<String, RandomState>::from_iter(order.iter().cloned());

    let surplus_tags = order_as_set
        .difference(&parsed_order_tags)
        .collect::<Vec<_>>();
    if surplus_tags.len() > 0 {
        bail!(
            "Manuálně zadané pořadí obsahuje tagy navíc ({:?})",
            surplus_tags
        );
    }

    let not_represented_tags = parsed_order_tags
        .difference(&order_as_set)
        .collect::<Vec<_>>();
    if not_represented_tags.len() > 0 {
        bail!(
            "V manuálně zadaném pořadí chybí tagy({:?})",
            not_represented_tags
        );
    }

    Ok(order)
}
