use std::path::PathBuf;

use anyhow::Result;
use iced::Element;
use iced::window::{self, Id, Settings};
use iced::{Subscription, Task};
use sqlx::SqlitePool;

mod components;
mod config;
mod error_screen;
mod pick_playlist;
mod update;

const PROGRAM_NAME: &str = "Ekkles";

/// Prasárna, ale proteď stačí
const DB_PATH: &str = "ekkles_data/db/database.sqlite3";

#[derive(Debug)]
/// Jednotlivé obrazovky aplikace
enum Screen {
    /// Vybírání playlistu k editaci
    PickPlaylist(pick_playlist::PlaylistPicker),
    /// Nastala nezotavitelná chyba
    ErrorOccurred(String),
    /// Editování playlistu
    EditPlaylist,
}

struct Ekkles {
    main_window_id: Id,
    db: SqlitePool,
    screen: Screen,
}

impl Screen {
    fn view(&self) -> Element<Message> {
        match self {
            Screen::PickPlaylist(picker) => picker.view().map(|msg| msg.into()),
            Screen::ErrorOccurred(err) => error_screen::view(err),
            Screen::EditPlaylist => todo!(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    /// Z aplikace bylo vyžádáno ukončení programu
    ShouldQuit,
    /// Bylo otevřeno hlavní okno, spouští se na začátku
    WindowOpened(Id),
    /// Bylo zavřeno hlavní okno, měli bychom ukončit prezentování
    WindowClosed(Id),
    // Message z obrazovky "PlaylistPicker"
    PlaylistPicker(pick_playlist::Message),
    /// Nastala chyba (ukládat pouhou String reprezentaci je ošklivé, ale [`anyhow::Error`]
    /// neimplementuje [`Clone`] a [`Message`] musí být `Clone`)
    ErrorOccured(String),
}

impl Ekkles {
    fn new() -> (Self, Task<Message>) {
        let (id, open_window_task) = window::open(Settings::default());

        let async_rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Nelze sestrojit async runtime");
        let db = async_rt
            .block_on(config::connect_db(PathBuf::from(DB_PATH)))
            .expect("Nelze se připojit k databázi");

        (
            Self {
                main_window_id: id,
                db,
                screen: Screen::PickPlaylist(pick_playlist::PlaylistPicker::new()),
            },
            open_window_task.map(|id| Message::WindowOpened(id)),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::window::close_events().map(|id| Message::WindowClosed(id))
    }

    fn view(&self, _window_id: Id) -> Element<Message> {
        self.screen.view()
    }

    fn title(&self, _window_id: Id) -> String {
        String::from(PROGRAM_NAME)
    }
}

fn main() -> iced::Result {
    // Inicializace loggeru
    pretty_env_logger::init();

    // Hlavní event-loop
    iced::daemon(Ekkles::title, Ekkles::update, Ekkles::view)
        .subscription(Ekkles::subscription)
        .run_with(Ekkles::new)
}
