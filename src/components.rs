use iced::{
    Length,
    widget::{Container, container, sensor, text},
};

pub mod playlist_item_styles;

#[derive(Debug)]
pub struct LazyLoadable<T, M>
where
    M: Clone,
{
    state: LazyLoadableState<T>,
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
            LazyLoadableState::Loading => container(text("Načítám obsah z databáze"))
                .center(Length::Fill)
                .style(container::secondary),
            LazyLoadableState::Loaded(_) => panic!(
                "Metoda view_not_loaded() nemůže být zavoláno na LazyLoadable ve stavu Loaded"
            ),
        }
    }

    pub fn start_loading(&mut self) {
        assert!(
            self.state.is_cold(),
            "Metoda start_loading() musí být zavolána na LazyLoadable ve stavu Cold"
        );

        self.state = LazyLoadableState::Loading
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
    Loading,
    Loaded(T),
}

impl<T> LazyLoadableState<T> {
    /// Returns `true` if the lazy loadable state is [`Cold`].
    ///
    /// [`Cold`]: LazyLoadableState::Cold
    #[must_use]
    pub fn is_cold(&self) -> bool {
        matches!(self, Self::Cold)
    }

    /// Returns `true` if the lazy loadable state is [`Loading`].
    ///
    /// [`Loading`]: LazyLoadableState::Loading
    #[must_use]
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    pub fn as_loaded(&self) -> Option<&T> {
        if let Self::Loaded(v) = self {
            Some(v)
        } else {
            None
        }
    }
}
