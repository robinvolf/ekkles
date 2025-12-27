//! Uchovává znovuvyužitelné komponenty, které se používají na více místech v programu

use std::fmt::Display;

use iced::{
    Length,
    task::Handle,
    widget::{Container, container, sensor, text},
};

pub mod playlist_item_styles;
pub mod song_picker;

/// V programu je několikrát používán [`iced::widget::combo_box`], jehož položky musí
/// implementovat [`Display`]. Nám ovšem při výběru jde zpravidla o `id`.
#[derive(Debug, Clone)]
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

    pub fn view_not_loaded(&self) -> Container<M> {
        match &self.state {
            LazyLoadableState::Cold => container(
                sensor(text("Načítám obsah z databáze"))
                    .on_show(|_| self.msg_start_loading.clone()),
            )
            .center(Length::Fill)
            .style(container::secondary),
            LazyLoadableState::Loading(_) => container(text("Načítám obsah z databáze"))
                .center(Length::Fill)
                .style(container::secondary),
            LazyLoadableState::Loaded(_) => panic!(
                "Metoda view_not_loaded() nemůže být zavoláno na LazyLoadable ve stavu Loaded"
            ),
        }
    }

    pub fn start_loading(&mut self, handle: Handle) {
        assert!(
            self.state.is_cold(),
            "Metoda start_loading() musí být zavolána na LazyLoadable ve stavu Cold"
        );

        self.state = LazyLoadableState::Loading(handle)
    }

    /// Pokud byl ve stavu `Loading`, taks zruší načítání daného zdroje přes jeho [`Handle`]. Vždy na konci nastaví stav na [`LazyLoadableState::Cold`]
    pub fn cancel_loading_opt(&mut self) {
        if let LazyLoadableState::Loading(handle) = &self.state {
            handle.abort();
        }
        self.state = LazyLoadableState::Cold;
    }

    pub fn finish_loading(&mut self, result: T) {
        assert!(
            self.state.is_loading(),
            "Metoda finish_loading() musí být zavolána na LazyLoadable ve stavu Loading"
        );

        self.state = LazyLoadableState::Loaded(result)
    }
}

#[derive(Debug)]
pub enum LazyLoadableState<T> {
    Cold,
    Loading(Handle),
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
        matches!(self, Self::Loading(_))
    }

    pub fn as_loaded(&self) -> Option<&T> {
        if let Self::Loaded(v) = self {
            Some(v)
        } else {
            None
        }
    }
}
