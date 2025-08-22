use std::fmt::Display;

use anyhow::{Context, Result, anyhow, bail};
use ekkles_data::{
    bible::{
        get_available_translations,
        indexing::{Book, Passage, VerseIndex, chapters_in_book, verses_in_chapter},
    },
    playlist::PlaylistMetadata,
};
use iced::{
    Alignment, Element, Length, Task,
    widget::{
        self, button, column, container, pick_list, row, scrollable, text, text_input,
        vertical_space,
    },
};
use log::{debug, trace};

use crate::{Ekkles, Screen, playlist_editor::PlaylistEditor};

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
    SelectionChanged,
    SetPreview(Passage),
    ClearPreview,
    PickPassage,
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
    preview: Option<Passage>,
    err_msg: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TranslationPickerItem {
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
            preview: None,
            err_msg: String::new(),
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
            text("až").width(Length::FillPortion(1)).center(),
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

        let passage_preview = match &self.preview {
            Some(passage) => {
                let preview_text = passage
                    .get_verses()
                    .iter()
                    .map(|(verse_number, text)| format!("{verse_number}: {text}\n"))
                    .collect::<String>();
                trace!("Preview vypadá takto:\n{}", preview_text);
                container(scrollable(text(preview_text)))
            }
            None => container(vertical_space()),
        };

        let submit_button = column![
            button("Vybrat")
                .style(widget::button::success)
                .on_press(Message::PickPassage)
                .width(Length::Fill),
            text(&self.err_msg)
                .style(widget::text::danger)
                .width(Length::Fill)
                .center()
        ]
        .spacing(10);

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
                column![
                    upper_picker,
                    detailed_picker,
                    passage_preview.height(200),
                    submit_button
                ]
                .spacing(100)
                .align_x(Alignment::Center)
                .width(Length::FillPortion(2)),
                container("").width(Length::FillPortion(1))
            ]
            .padding(10)
            .height(Length::Fill)
            .align_y(Alignment::Center),
        ))
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
                Task::done(Message::SelectionChanged.into())
            }
            Message::FromBookPicked(book) => {
                debug!("Vybrána kniha (od) {}", book);
                picker.picked_from_book = Some(book);
                picker.picked_from_chapter = None;
                picker.picked_from_verse = None;
                Task::done(Message::SelectionChanged.into())
            }
            Message::FromChapterPicked(chapter) => {
                debug!("Vybrána kapitola (od) {}", chapter);
                picker.picked_from_chapter = Some(chapter);
                picker.picked_from_verse = None;
                Task::done(Message::SelectionChanged.into())
            }
            Message::FromVersePicked(verse) => {
                debug!("Vybrán verš (od) {}", verse);
                picker.picked_from_verse = Some(verse);
                Task::done(Message::SelectionChanged.into())
            }
            Message::ToBookPicked(book) => {
                debug!("Vybrána kniha (do) {}", book);
                picker.picked_to_book = Some(book);
                picker.picked_to_chapter = None;
                picker.picked_to_verse = None;
                Task::done(Message::SelectionChanged.into())
            }
            Message::ToChapterPicked(chapter) => {
                debug!("Vybrána kapitola (do) {}", chapter);
                picker.picked_to_chapter = Some(chapter);
                picker.picked_to_verse = None;
                Task::done(Message::SelectionChanged.into())
            }
            Message::ToVersePicked(verse) => {
                debug!("Vybrán verš (do) {}", verse);
                picker.picked_to_verse = Some(verse);
                Task::done(Message::SelectionChanged.into())
            }
            Message::ReturnToEditor => {
                debug!("Vracím do editoru playlistů");
                state.screen = Screen::EditPlaylist(PlaylistEditor::new(picker.playlist.clone()));
                Task::done(crate::playlist_editor::Message::LoadSongNameCache.into())
            }
            Message::PickPassage => match picker.validate() {
                Ok((from, to)) => {
                    debug!(
                        "Pasáž úspěšně zvalidována, přidávám ji na konec playlistu a vracím se do editoru"
                    );
                    picker.playlist.push_bible_passage(
                        picker
                            .picked_translation
                            .as_ref()
                            .expect("Pasáž byla validována, musí být vybrán překlad")
                            .id,
                        from,
                        to,
                    );

                    Task::done(Message::ReturnToEditor.into())
                }
                Err(err) => {
                    debug!("Pasáž není validní, zobrazuji chybovou hlášku");
                    picker.err_msg = err.to_string();
                    Task::none()
                }
            },
            Message::SelectionChanged => match picker.validate() {
                Ok((from, to)) => {
                    trace!("Detekována validní pasáž, načítám preview");
                    let conn = state.db.acquire();
                    let translation_id = picker
                        .picked_translation
                        .as_ref()
                        .expect("Pasáž byla validována, musí být vybrán překlad")
                        .id;
                    Task::perform(
                        async move {
                            let mut conn = conn.await?;
                            Passage::load(from, to, translation_id, &mut conn).await
                        },
                        |res| match res {
                            Ok(passage) => Message::SetPreview(passage).into(),
                            Err(e) => crate::Message::FatalErrorOccured(format!("{:?}", e)),
                        },
                    )
                }
                Err(_) => {
                    trace!("Pasáž není validní, vyčišťuji preview");
                    Task::done(Message::ClearPreview.into())
                }
            },
            Message::SetPreview(passage) => {
                debug!("Nastavena pasáž pro preview");
                picker.preview = Some(passage);
                Task::none()
            }
            Message::ClearPreview => {
                debug!("Mažu preview");
                picker.preview = None;
                Task::none()
            }
        }
    }

    /// Zvaliduje, že pasáž je korektně vybraná. Kapitola, kniha i verš jsou legální
    /// v obou případech a jsou ve správném pořadí. Také zkontroluje,
    /// že byl vybrán překlad. Pokud cokoliv z tohoto není splněno, vrací Error.
    /// Pokud validace proběhne úspěšně vrací dvojici indexů do bible `from` a `to`.
    fn validate(&self) -> Result<(VerseIndex, VerseIndex)> {
        const CONTEXT_MSG: &str = "Pasáž ještě není vybraná celá";

        if self.picked_translation.is_none() {
            bail!("Nebyl vybrán příslušný překlad");
        }

        let from = VerseIndex::try_new(
            self.picked_from_book.context(CONTEXT_MSG)?,
            self.picked_from_chapter.context(CONTEXT_MSG)?,
            self.picked_from_verse.context(CONTEXT_MSG)?,
        )
        .context("Neplatný začátek pasáže")?;

        let to = VerseIndex::try_new(
            self.picked_to_book.context(CONTEXT_MSG)?,
            self.picked_to_chapter.context(CONTEXT_MSG)?,
            self.picked_to_verse.context(CONTEXT_MSG)?,
        )
        .context("Neplatný konec pasáže")?;

        if from > to {
            Err(anyhow!("Začátek pasáže se nachází až za koncem"))
        } else {
            Ok((from, to))
        }
    }
}
