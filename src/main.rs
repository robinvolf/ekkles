use iced::widget::{column, container, text};
use iced::window::{self, Id, Settings};
use iced::Task;
use iced::{widget::button, Element};
use log::{debug, info};

const PROGRAM_NAME: &str = "Ekkles";

#[derive(Debug)]
enum Mode {
    /// Prezentující mód s novým oknem, uchovává ID nového okna
    Presenting(Id),
    /// Běžný režim
    Normal,
}

struct Ekkles {
    main_window_id: Id,
    mode: Mode,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    /// Způsobí ukončení programu
    Quit,
    /// Vytvoří nové okno a spustí prezentační režim
    Present,
    /// Bylo otevřeno hlavní okno, spouští se na začátku
    MainWindowOpened,
    /// Došlo k vytvoření prezentujícího okna
    PresentWindowOpened,
    /// Došlo k zavření prezentujícího okna
    PresentWindowClosed,
    /// Zavři prezentující okno
    StopPresenting,
}

impl Ekkles {
    fn new() -> (Self, Task<Message>) {
        let (id, task) = window::open(Settings::default());

        (
            Self {
                main_window_id: id,
                mode: Mode::Normal,
            },
            task.map(|_| Message::MainWindowOpened),
        )
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        debug!("Přišla zpráva: {:?}", msg);
        match msg {
            Message::Quit => {
                info!("Ukončuji program");
                iced::exit()
            }
            Message::Present => {
                let (id, task) = window::open(Settings::default());
                self.mode = Mode::Presenting(id);
                task.map(|_| Message::PresentWindowOpened)
            }
            // Prozatím otevření a zavření prezentujícího okna ignorujeme
            Message::PresentWindowOpened => Task::none(),
            Message::PresentWindowClosed => Task::none(),
            Message::MainWindowOpened => Task::none(),
            Message::StopPresenting => {
                if let Mode::Presenting(id) = self.mode {
                    self.mode = Mode::Normal;
                    window::close(id).map(|_: Task<Id>| Message::PresentWindowClosed)
                } else {
                    unreachable!(
                        "Ukončit prezentaci je možné pouze pokud jsme v prezentujícím módu"
                    )
                }

                // Tady se resetujeme -> přejdeme zpátky do Normal módu
            }
        }
    }

    fn view(&self, window_id: Id) -> Element<Message> {
        if window_id == self.main_window_id {
            match self.mode {
                Mode::Presenting(_) => container(column![
                    button("Vypnout").on_press(Message::Quit),
                    button("Zavřít prezentující okno").on_press(Message::StopPresenting)
                ])
                .into(),
                Mode::Normal => container(column![
                    button("Vypnout").on_press(Message::Quit),
                    button("Otevřít prezentující okno").on_press(Message::Present)
                ])
                .into(),
            }
        } else {
            text("Ahoj, tady je prezentace").into()
        }
    }

    fn title(&self, window_id: Id) -> String {
        match self.mode {
            Mode::Presenting(id) if window_id == id => String::from("Prezentace"),
            Mode::Normal | Mode::Presenting(_) => String::from(PROGRAM_NAME),
        }
    }
}

fn main() -> iced::Result {
    // Inicializace loggeru
    pretty_env_logger::init();

    // Hlavní event-loop
    iced::daemon(Ekkles::title, Ekkles::update, Ekkles::view).run_with(Ekkles::new)
}
