//! Uchovává znovuvyužitelné komponenty, které se používají na více místech v programu

use std::{
    fmt::Display,
    sync::atomic::{AtomicU32, Ordering},
};

use iced::{
    Length,
    task::Handle,
    widget::{Container, container, sensor, text},
};
use log::trace;

pub mod bible_picker;
pub mod playlist_item_styles;
pub mod shortcuts;
pub mod song_picker;

/// V programu je několikrát používán [`iced::widget::combo_box()`], jehož položky musí
/// implementovat [`Display`]. Nám ovšem při výběru jde zpravidla o `id`.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerItem {
    pub(crate) id: i64,
    pub(crate) name: String,
}

impl Display for PickerItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

impl PickerItem {
    fn new(id: i64, name: String) -> Self {
        Self { id, name }
    }
}

/// V programu se na několika místech používá tento vzor:
/// - Mám zdroj, který je nutné načíst z databáze
/// - Jakmile je obrazovka, na které je zdroj potřeba, zobrazena, měl by se zdroj začít načítat
/// - Jakmile je zdroj načten, může se zobrazit na něm závislý obsah
///
/// Tato struktura se používá pro tento účel.
#[derive(Debug)]
pub struct LazyLoadable<T, M>
where
    M: Clone,
{
    /// Stav (nenačítá se/načítá se/načteno)
    state: LazyLoadableState<T>,
    /// Message, která se vyvolá při zobrazení nenačítajícího se `LazyLoadable`
    msg_start_loading: M,
}

impl<T, M> LazyLoadable<T, M>
where
    M: Clone,
{
    pub fn new(msg_start_loading: M) -> Self {
        LazyLoadable {
            state: LazyLoadableState::Cold,
            msg_start_loading,
        }
    }

    pub fn state(&self) -> &LazyLoadableState<T> {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut LazyLoadableState<T> {
        &mut self.state
    }

    pub fn invalidate(&mut self) {
        self.state = LazyLoadableState::Cold;
    }

    pub fn view_not_loaded(&self) -> Container<M> {
        let text = text("Načítám obsah z databáze").wrapping(text::Wrapping::None);

        match &self.state {
            LazyLoadableState::Cold => {
                container(sensor(text).on_show(|_| self.msg_start_loading.clone()))
                    .center(Length::Shrink)
                    .style(container::secondary)
            }
            LazyLoadableState::Loading { .. } => container(text)
                .center(Length::Shrink)
                .style(container::secondary),
            LazyLoadableState::Loaded(_) => panic!(
                "Metoda view_not_loaded() nemůže být zavoláno na LazyLoadable ve stavu Loaded"
            ),
        }
    }

    /// Začne načítat zdroj s příslušným `Handle` a vrátí `id`, které je třeba vrátit při [`Self::finish_loading()`], jakmile bude `Task` dokončen. Pokud je již ve stavu `Loading`, nic se nestane.
    pub fn start_loading(&mut self, handle: Handle) -> Option<u32> {
        static TASK_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

        match self.state {
            LazyLoadableState::Cold => {
                let task_id = TASK_ID_COUNTER.fetch_add(1, Ordering::Relaxed);

                self.state = LazyLoadableState::Loading { handle, task_id };
                Some(task_id)
            }
            LazyLoadableState::Loading { .. } => None,
            LazyLoadableState::Loaded(_) => {
                panic!("Metoda start_loading() nesmí být zavolána na LazyLoadable ve stavu Loaded")
            }
        }
    }

    /// Pokud byl ve stavu `Loading`, tak zruší načítání daného zdroje přes jeho [`Handle`]. Vždy na konci nastaví stav na [`LazyLoadableState::Cold`]
    pub fn cancel_loading_opt(&mut self) {
        if let LazyLoadableState::Loading { handle, task_id: _ } = &self.state {
            handle.abort();
        }
        self.state = LazyLoadableState::Cold;
    }

    /// Ukončí načítání a nastaví `LazyLoadable` do stavu `Loaded`, pokud nebyl task s `handle` zrušen ([`Handle::is_aborted()`]) a `task_id` odpovídá `id`, které [`Self::start_loading()`] vrátil jako poslední
    pub fn finish_loading(&mut self, result: T, task_id: u32) {
        if let LazyLoadableState::Loading {
            handle,
            task_id: desired_id,
        } = &self.state
            && !handle.is_aborted()
        {
            if task_id == *desired_id {
                self.state = LazyLoadableState::Loaded(result)
            } else {
                trace!(
                    "LazyLoadable obdržel finish_loading(), pro id {task_id}, ale čeká na task s id {desired_id}, zahazuji výsledek"
                );
            }
        }
    }
}

#[derive(Debug)]
pub enum LazyLoadableState<T> {
    Cold,
    Loading {
        /// Handle pro daný [`iced::task::Task`], aby bylo možné jej zrušit
        handle: Handle,
        /// Id, podle kterého poznáme, který `Task` se načítal (při příliš rychlém vyřízení `Task`u) jeho zrušení neproběhne (už je vykonán) a potom je nutné rozlišit, poslední vyvolaný `Task` pro daný `LazyLoadableState` pomocí `task_id` a ostatní zahodit
        task_id: u32,
    },
    Loaded(T),
}

impl<T> LazyLoadableState<T> {
    /// Vrací `true` pokud je ve stavu [`Cold`].
    ///
    /// [`Cold`]: LazyLoadableState::Cold
    #[must_use]
    pub fn is_cold(&self) -> bool {
        matches!(self, Self::Cold)
    }

    /// Vrací `true` pokud je ve stavu [`Loading`].
    ///
    /// [`Loading`]: LazyLoadableState::Loading
    #[must_use]
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    pub fn as_loaded(&self) -> Option<&T> {
        if let Self::Loaded(v) = self {
            Some(v)
        } else {
            None
        }
    }

    /// Vrací `true` pokud je ve stavu [`Loaded`].
    ///
    /// [`Loaded`]: LazyLoadableState::Loaded
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded(..))
    }
}

#[derive(Debug)]
pub enum OpenedPicker {
    Song(song_picker::SongPicker),
    Passage(bible_picker::BiblePicker),
    None,
}

impl OpenedPicker {
    pub fn as_song_mut(&mut self) -> Option<&mut song_picker::SongPicker> {
        if let Self::Song(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_passage_mut(&mut self) -> Option<&mut bible_picker::BiblePicker> {
        if let Self::Passage(v) = self {
            Some(v)
        } else {
            None
        }
    }
}
