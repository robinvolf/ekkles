use iced::{
    Subscription,
    keyboard::{Key, Modifiers, key},
    widget::{Column, column, text},
};
use log::{trace, warn};

/// Reprezentuje deklarativně definovanou klávesovou zkratku. Takto lze jednoduše deklarovat
/// klávesové zkratky na jednom místě v aplikaci a dostaneme nápovědu + Messages při stisku _zadarmo™_.
#[derive(Clone, Debug, Hash)]
pub struct KeyboardShortcut<S>
where
    S: Clone + Send + Sync + std::hash::Hash + 'static,
{
    key: Key,
    mods: Modifiers,
    on_press: S,
    help_msg: &'static str,
}

impl<S> KeyboardShortcut<S>
where
    S: Clone + Send + Sync + std::hash::Hash + 'static,
{
    /// - `key` Klávesa, která se musí stisknout
    /// - `mods` Modifikátory, které musí být stisknuté
    /// - `on_press` Hodnota, která se předá Subscription při dané zkratce
    /// - `help_msg` Krátký textový řetězec, který pro uživatele popíše, co klávesová zkratka dělá
    pub fn new(key: Key, mods: Modifiers, on_press: S, help_msg: &'static str) -> Self {
        Self {
            key,
            mods,
            on_press,
            help_msg,
        }
    }

    /// Vytvoří [`Subscription`] pro dané klávesové zkratky.
    ///
    /// # Pořadí
    /// Pokud je klávesa použita alespoň 2x a to s modifierem a bez něj, akce s největším
    /// počtem modifierů se musí vyskytovat první v seznamu.
    ///
    /// # Proč ne reference?
    /// `shortcuts` není reference, protože by jinak reference unikla z této funkce skrz `Subscription`,
    /// tudíž se předá přímo `array` referencí.
    pub fn subscription<const N: usize>(shortcuts: [Self; N]) -> Subscription<S> {
        iced::keyboard::listen()
            .with(shortcuts)
            .filter_map(move |(shortcuts, event)| {
                if let iced::keyboard::Event::KeyPressed { key, modifiers, .. } = event {
                    trace!("Přišel event z klávesnice: {:?}", (key.as_ref(), modifiers));
                    shortcuts.iter().find_map(|shortcut| {
                        if key == shortcut.key
                            && (!modifiers.intersection(shortcut.mods).is_empty()
                                || shortcut.mods.is_empty())
                        {
                            Some(shortcut.on_press.clone())
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            })
    }

    /// Vytvoří sloupec s nápovědou. Generický parametr `M` je tady, aby type-checker byl šťastný.
    pub fn view<M>(shortcuts: &[Self]) -> Column<M> {
        column(shortcuts.iter().map(|s| text(s.help_string()).into()))
    }

    /// Vytvoří textovou reprezentaci pro nápovědu
    fn help_string(&self) -> String {
        let key = match &self.key {
            Key::Named(key::Named::ArrowDown) => "↓",
            Key::Named(key::Named::ArrowUp) => "↑",
            Key::Named(key::Named::Escape) => "ESC",
            Key::Named(k) => {
                warn!("Náhled pro neznámou klávesu: {:?}", k);
                "?"
            }
            Key::Character(c) => c.as_str(),
            Key::Unidentified => "?",
        };

        let modifiers = self
            .mods
            .iter_names()
            .map(|(name, _modifier)| String::from("+") + name)
            .collect::<String>();

        let help = self.help_msg;

        format!("{key} {modifiers} {help}")
    }
}
