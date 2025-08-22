use std::{fmt::Display, time::Duration};

use ekkles_data::{
    bible::{
        get_available_translations,
        indexing::{Book, chapters_in_book, verses_in_chapter},
    },
    playlist::PlaylistMetadata,
};
use iced::{
    Alignment, Color, Element, Length, Task,
    widget::{button, column, container, pick_list, row, text, text_input},
};
use log::debug;
use tokio::time::sleep;

use crate::{Ekkles, Screen};

#[derive(Debug, Clone)]
pub enum Message {
    LoadTranslations,
    TranslationsLoaded(Vec<TranslationPickerItem>),
    TranslationPicked(TranslationPickerItem),
    FromBookPicked(Book),
    FromChapterPicked(u8),
    FromVersePicked(u8),
    ToBookPicked(Book),
    ToChapterPicked(u8),
    ToVersePicked(u8),
    ReturnToEditor,
}

impl From<Message> for crate::Message {
    fn from(value: Message) -> Self {
        crate::Message::BiblePicker(value)
    }
}

#[derive(Debug)]
pub struct BiblePicker {
    playlist: PlaylistMetadata,
    translations: Option<Vec<TranslationPickerItem>>,
    picked_translation: Option<TranslationPickerItem>,
    picked_from_book: Option<Book>,
    picked_from_chapter: Option<u8>,
    picked_from_verse: Option<u8>,
    picked_to_book: Option<Book>,
    picked_to_chapter: Option<u8>,
    picked_to_verse: Option<u8>,
}

#[derive(Debug, Clone, PartialEq)]
struct TranslationPickerItem {
    id: i64,
    name: String,
}

impl Display for TranslationPickerItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

impl BiblePicker {
    pub fn new(playlist: PlaylistMetadata) -> Self {
        Self {
            playlist,
            translations: None,
            picked_translation: None,
            picked_from_book: None,
            picked_from_chapter: None,
            picked_from_verse: None,
            picked_to_book: None,
            picked_to_chapter: None,
            picked_to_verse: None,
        }
    }

    pub fn view(&self) -> Element<Message> {
        let upper_picker = row![
            pick_list(
                // TODO: Opravdu je tu nutné klonovat?
                self.translations.clone().unwrap_or(vec![]),
                self.picked_translation.clone(),
                Message::TranslationPicked,
            )
            .placeholder(if self.translations.is_some() {
                "Vyber překlad"
            } else {
                "Načítám překlady..."
            })
            .width(Length::FillPortion(1)),
            text_input("Např. Jan 3:4 - 4:5", "").width(Length::FillPortion(3))
        ];

        let detailed_picker = row![
            pick_list(
                ekkles_data::bible::indexing::BIBLE_BOOKS,
                self.picked_from_book,
                Message::FromBookPicked
            )
            .placeholder("Kniha")
            .width(Length::FillPortion(3)),
            match self.picked_from_book {
                Some(book) => pick_list(
                    chapters_in_book(book).collect::<Vec<u8>>(),
                    self.picked_from_chapter,
                    Message::FromChapterPicked
                )
                .placeholder("Kapitola"),
                None => pick_list(vec![], self.picked_from_chapter, Message::FromChapterPicked)
                    .placeholder("Vyber knihu"),
            }
            .width(Length::FillPortion(1)),
            match (self.picked_from_book, self.picked_from_chapter) {
                (Some(book), Some(chapter)) => pick_list(
                    verses_in_chapter(book, chapter)
                        .unwrap()
                        .collect::<Vec<u8>>(),
                    self.picked_from_verse,
                    Message::FromVersePicked
                )
                .placeholder("Verš"),
                _ => pick_list(vec![], self.picked_from_chapter, Message::FromVersePicked)
                    .placeholder("Vyber kapitolu"),
            }
            .width(Length::FillPortion(1)),
            text("až").width(Length::FillPortion(1)),
            pick_list(
                ekkles_data::bible::indexing::BIBLE_BOOKS,
                self.picked_to_book,
                Message::ToBookPicked
            )
            .placeholder("Kniha")
            .width(Length::FillPortion(3)),
            match self.picked_to_book {
                Some(book) => pick_list(
                    chapters_in_book(book).collect::<Vec<u8>>(),
                    self.picked_to_chapter,
                    Message::ToChapterPicked
                )
                .placeholder("Kapitola"),
                None => pick_list(vec![], self.picked_to_chapter, Message::ToChapterPicked)
                    .placeholder("Vyber knihu"),
            }
            .width(Length::FillPortion(1)),
            match (self.picked_to_book, self.picked_to_chapter) {
                (Some(book), Some(chapter)) => pick_list(
                    verses_in_chapter(book, chapter)
                        .unwrap()
                        .collect::<Vec<u8>>(),
                    self.picked_to_verse,
                    Message::ToVersePicked
                )
                .placeholder("Verš"),
                _ => pick_list(vec![], self.picked_to_chapter, Message::ToVersePicked)
                    .placeholder("Vyber kapitolu"),
            }
            .width(Length::FillPortion(1)),
        ];

        let passage_preview = text("Tady bude preview vybraných veršů");

        Into::<Element<Message>>::into(container(
            row![
                container(
                    button("Zpět")
                        .on_press(Message::ReturnToEditor)
                        .width(Length::Fill)
                )
                .align_bottom(Length::Fill)
                .width(Length::FillPortion(1))
                .padding(30),
                column![upper_picker, detailed_picker, passage_preview]
                    .spacing(10)
                    .align_x(Alignment::Center)
                    .width(Length::FillPortion(2)),
                container("").width(Length::FillPortion(1))
            ]
            .padding(10)
            .height(Length::Fill)
            .align_y(Alignment::Center),
        ))
        .explain(Color::BLACK)
    }

    pub fn update(state: &mut Ekkles, msg: Message) -> Task<crate::Message> {
        let picker = match &mut state.screen {
            Screen::PickBible(picker) => picker,
            screen => panic!(
                "Update pro BiblePicker zavolán nad jinou obrazovkou {:?}",
                screen
            ),
        };

        match msg {
            Message::LoadTranslations => {
                debug!("Načítám seznam překladů");
                let conn = state.db.acquire();
                Task::perform(
                    async {
                        let mut conn = conn.await?;
                        sleep(Duration::from_secs(3)).await; // TODO
                        get_available_translations(&mut conn).await
                    },
                    |res| match res {
                        Ok(translations) => {
                            let items = translations
                                .into_iter()
                                .map(|(id, name)| TranslationPickerItem { id, name })
                                .collect();
                            Message::TranslationsLoaded(items).into()
                        }
                        Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                    },
                )
            }
            Message::TranslationsLoaded(translations) => {
                debug!("Překlady načteny {:#?}", translations);
                picker.translations = Some(translations);
                Task::none()
            }
            Message::TranslationPicked(item) => {
                debug!("Byl vybrán překlad: {}", item);
                picker.picked_translation = Some(item);
                Task::none()
            }
            Message::FromBookPicked(book) => {
                debug!("Vybrána kniha (od) {}", book);
                picker.picked_from_book = Some(book);
                picker.picked_from_chapter = None;
                picker.picked_from_verse = None;
                Task::none()
            }
            Message::FromChapterPicked(chapter) => {
                debug!("Vybrána kapitola (od) {}", chapter);
                picker.picked_from_chapter = Some(chapter);
                picker.picked_from_verse = None;
                Task::none()
            }
            Message::FromVersePicked(verse) => {
                debug!("Vybrán verš (od) {}", verse);
                picker.picked_from_verse = Some(verse);
                Task::none()
            }
            Message::ToBookPicked(book) => {
                debug!("Vybrána kniha (do) {}", book);
                picker.picked_to_book = Some(book);
                picker.picked_to_chapter = None;
                picker.picked_to_verse = None;
                Task::none()
            }
            Message::ToChapterPicked(chapter) => {
                debug!("Vybrána kapitola (do) {}", chapter);
                picker.picked_to_chapter = Some(chapter);
                picker.picked_to_verse = None;
                Task::none()
            }
            Message::ToVersePicked(verse) => {
                debug!("Vybrán verš (do) {}", verse);
                picker.picked_to_verse = Some(verse);
                Task::none()
            }
            Message::ReturnToEditor => todo!(),
        }
    }
}
