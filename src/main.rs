use components::{bible_picker, song_picker};
use config::Config;
use iced::Element;
use iced::window::{self, Id, Settings};
use iced::{Subscription, Task};
use log::info;
use sqlx::SqlitePool;

mod components;
mod config;
mod error_screen;
mod playlist_editor;
mod presenter;
mod song_editor;
mod start_screen;
mod update;

const PROGRAM_NAME: &str = "Ekkles";

#[derive(Debug)]
/// Jednotlivé obrazovky aplikace
enum Screen {
    /// Vybírání playlistu k editaci
    StartScreen(start_screen::StartScreen),
    /// Nastala nezotavitelná chyba
    ErrorOccurred(String),
    /// Editování playlistu
    EditPlaylist(playlist_editor::PlaylistEditor),
    /// Prezentování playlistu
    Presenter(presenter::Presenter),
    /// Editace písní
    SongEditor(song_editor::Editor),
}

struct Ekkles {
    main_window_id: Id,
    db: SqlitePool,
    screen: Screen,
}

#[derive(Debug, Clone)]
enum Message {
    /// Z aplikace bylo vyžádáno ukončení programu
    ShouldQuit,
    /// Bylo otevřeno hlavní okno, spouští se na začátku
    WindowOpened(Id),
    /// Bylo zavřeno hlavní okno, měli bychom ukončit prezentování
    WindowClosed(Id),
    /// Message z obrazovky "PlaylistPicker"
    PlaylistPicker(start_screen::Message),
    /// Message z obrazovky "PlaylistEditor"
    PlaylistEditor(playlist_editor::Message),
    /// Message z obrazovky "SongPicker"
    SongPicker(song_picker::Message),
    /// Message z obrazovky "BiblePicker"
    BiblePicker(bible_picker::Message),
    /// Message z obrazovky "Presenter"
    Presenter(presenter::Message),
    /// Message z obrazovky SongEditor
    SongEditor(song_editor::Message),
    /// Nastala nezotavitelná chyba, měli bychom ukončit program. (ukládat pouhou String
    /// reprezentaci je ošklivé, ale [`anyhow::Error`] neimplementuje [`Clone`]
    /// a [`Message`] musí být `Clone`)
    FatalErrorOccured(String),
}

impl Ekkles {
    fn boot() -> (Self, Task<Message>) {
        let config = Config::new();
        info!("Bootuji ekkles s následující konfigurací: {:#?}", config);

        let (id, open_window_task) = window::open(Settings::default());

        let async_rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Nelze sestrojit async runtime");
        let db = async_rt
            .block_on(ekkles_data::database::open_or_create_database(
                config.db_path,
            ))
            .expect("Nelze se připojit k databázi");

        (
            Self {
                main_window_id: id,
                db,
                screen: Screen::StartScreen(start_screen::StartScreen::new()),
            },
            open_window_task.map(|id| Message::WindowOpened(id)),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        let window_closed_events = iced::window::close_events().map(|id| Message::WindowClosed(id));

        let screen_specific_events = match &self.screen {
            Screen::StartScreen(_) => Subscription::none(),
            Screen::ErrorOccurred(_) => Subscription::none(),
            Screen::EditPlaylist(editor) => editor.subscription(),
            Screen::Presenter(presenter) => presenter.subscription(),
            Screen::SongEditor(editor) => editor.subscription(),
        };

        Subscription::batch([window_closed_events, screen_specific_events])
    }

    fn view(&self, window_id: Id) -> Element<Message> {
        if window_id == self.main_window_id {
            match &self.screen {
                Screen::StartScreen(picker) => picker.view().map(|msg| msg.into()),
                Screen::ErrorOccurred(err) => error_screen::view(err),
                Screen::EditPlaylist(editor) => editor.view().map(|msg| msg.into()),
                Screen::Presenter(presenter) => presenter.view_control().map(|msg| msg.into()),
                Screen::SongEditor(editor) => editor.view().map(|msg| msg.into()),
            }
        } else if let Screen::Presenter(presenter) = &self.screen
            && presenter
                .get_presentation_window_id()
                .is_some_and(|id| id == window_id)
        {
            presenter.view_presentation().map(|msg| msg.into())
        } else {
            panic!(
                "Zavoláno view pro jiné než hlavní okno (id {window_id}) na obrazovce {:?}",
                self.screen
            );
        }
    }
}

fn main() -> iced::Result {
    // Inicializace loggeru
    pretty_env_logger::init();

    // Hlavní event-loop
    iced::daemon(Ekkles::boot, Ekkles::update, Ekkles::view)
        .subscription(Ekkles::subscription)
        .title(PROGRAM_NAME)
        .run()
}
