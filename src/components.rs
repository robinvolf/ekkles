use iced::{
    Element, Length,
    widget::{container, sensor, text},
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

    pub fn as_loaded(&self) -> Option<&T> {
        self.state.as_loaded()
    }

    pub fn view_if_not_loaded(&self) -> Option<impl Into<Element<M>>> {
        let not_loaded_content: container::Container<'_, M, iced::Theme, _> =
            container(text("Načítám obsah z databáze"))
                .center(Length::Fill)
                .style(container::secondary);
        match &self.state {
            LazyLoadableState::Cold => Some(
                sensor(not_loaded_content)
                    .on_show(|_| self.msg_start_loading.clone())
                    .into(),
            ),
            LazyLoadableState::Loading => Some(Into::<Element<M>>::into(not_loaded_content)),
            LazyLoadableState::Loaded(_) => None,
        }
    }

    pub fn start_loading(&mut self) {
        assert!(
            self.state.is_cold(),
            "start_loading() musí být zavolána na LazyLoadable ve stavu Cold"
        );

        self.state = LazyLoadableState::Loading
    }

    pub fn finish_loading(&mut self, result: T) {
        assert!(
            self.state.is_loading(),
            "finish_loading() musí být zavolána na LazyLoadable ve stavu Loading"
        );

        self.state = LazyLoadableState::Loaded(result)
    }
}

#[derive(Debug)]
enum LazyLoadableState<T> {
    Cold,
    Loading,
    Loaded(T),
}

impl<T> LazyLoadableState<T> {
    /// Returns `true` if the lazy loadable state is [`Cold`].
    ///
    /// [`Cold`]: LazyLoadableState::Cold
    #[must_use]
    fn is_cold(&self) -> bool {
        matches!(self, Self::Cold)
    }

    /// Returns `true` if the lazy loadable state is [`Loading`].
    ///
    /// [`Loading`]: LazyLoadableState::Loading
    #[must_use]
    fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    fn as_loaded(&self) -> Option<&T> {
        if let Self::Loaded(v) = self {
            Some(v)
        } else {
            None
        }
    }
}
