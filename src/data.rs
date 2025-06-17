//! Modul pro uchovávání datových struktur reprezentující:
//! - Písně
//! - Playlisty
//! - Bible
//!
//! Zatím je to tu masivní TODO!

use anyhow::{Result, bail};
use std::collections::HashMap;

pub mod bible;
pub mod db;
pub mod xml_song;

/// Tag označující část písně, typicky něco jako "V1", "V2", "C" (sloka1, sloka2, refrén)
pub type PartTag = String;

/// Píseň
///
/// ### Invarianty
/// - Vektor `order` musí obsahovat *pouze* `PartTag`, které se nacházejí jako klíče v `parts`
#[derive(Debug, PartialEq, Eq)]
pub struct Song {
    /// Název písně
    title: String,
    /// Volitelný autor písně
    author: Option<String>,
    /// Jednotlivé části písně "adresované" Tagem
    parts: HashMap<PartTag, String>,
    /// Pořadí jednotlivých částí písně, umožňuje opakování jedné části
    order: Vec<PartTag>,
}

impl Song {
    /// Zkontroluje invariant, že všechny položky pořadí `order` jsou validní
    /// tagy nacházející se v `parts`. Pokud ne, vrátí Error.
    fn check_order_validity(&self) -> Result<()> {
        for tag in self.order.iter() {
            if !self.parts.contains_key(tag) {
                bail!(
                    "Píseň {} má v pořadí tag {}, ale ve slovech jej neobsahuje",
                    self.title,
                    tag
                );
            }
        }

        Ok(())
    }
}
