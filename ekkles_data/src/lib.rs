//! Modul pro uchovávání datových struktur reprezentující:
//! - Písně
//! - Playlisty
//! - Bible
//!
//! Zatím je to tu masivní TODO!

use anyhow::{Result, bail};
use std::collections::{HashMap, HashSet};

pub mod bible;
pub mod song_db;
pub mod song_xml;

/// Tag označující část písně, typicky něco jako "V1", "V2", "C" (sloka1, sloka2, refrén)
pub type PartTag = String;

/// Píseň
///
/// ### Invarianty
/// - Klíče v `parts` a položky vektoru `ordered` musejí být totožné
/// - Jednotlivé položky vektoru `order` nesmí obsahovat znak mezery ` `
#[derive(Debug, PartialEq, Eq)]
pub struct Song {
    /// Název písně
    pub title: String,
    /// Volitelný autor písně
    pub author: Option<String>,
    /// Jednotlivé části písně "adresované" Tagem
    pub parts: HashMap<PartTag, String>,
    /// Pořadí jednotlivých částí písně, umožňuje opakování jedné části
    pub order: Vec<PartTag>,
}

impl Song {
    /// Zkontroluje invarianty, viz dokumentace [Song]. Pokud je nějaký invariant
    /// nesplněn, vrací Error s popisem chyby.
    fn check_invariants(&self) -> Result<()> {
        let tags_from_parts: HashSet<_> = self.parts.keys().collect();
        let tags_from_order: HashSet<_> = self.order.iter().collect();

        if tags_from_order != tags_from_parts {
            bail!(
                "Píseň {} má odlišné tagy ve slovech ({:?}) a v pořadí ({:?})",
                self.title,
                tags_from_parts,
                tags_from_order
            );
        }

        for tag in tags_from_parts {
            if tag.contains(' ') {
                bail!("Píseň {} obsahuje tag s mezerou '{}'", self.title, tag);
            }
        }

        Ok(())
    }
}

enum PlaylistItem {
    BiblePassage,
    Song(Song),
}

struct Playlist {
    id: i64,
    items: Vec<PlaylistItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_invariants_test_space() {
        let song = Song {
            title: String::from("Haleluja (Svatý Pán Bůh Všemohoucí)"),
            author: None,
            parts: HashMap::from([
                (
                    String::from("C"),
                    String::from("Haleluja, haleluja,\nvládne nám všemocný Bůh a Král."),
                ),
                (
                    String::from("V1a"),
                    String::from(
                        "Haleluja, Svatý, Svatý,\nSvatý Pán Bůh Všemohoucí,\nhoden je On sám,\nBeránek, náš Pán,\npřijmout chválu,",
                    ),
                ),
                (
                    String::from("V1b"),
                    String::from(
                        "Svatý, Svatý Pán Bůh Všemohoucí,\nhoden je On sám,\nBeránek, náš Pán,\npřijmout chválu.",
                    ),
                ),
                (
                    String::from("V2a"),
                    String::from(
                        "Haleluja, Svatý, Svatý,\nTy jsi náš Bůh Všemohoucí,\npřijmi, Pane náš,\npřijmi, Pane náš,\nnaši chválu,",
                    ),
                ),
                (
                    String::from("V2b"),
                    String::from(
                        "Svatý, Ty jsi náš Bůh Všemohoucí,\npřijmi, Pane náš,\npřijmi, Pane náš,\nchválu.",
                    ),
                ),
                (
                    String::from("TAG S MEZERAMI"),
                    String::from("Smyšlená slova"),
                ),
            ]),
            order: vec![
                String::from("C"),
                String::from("V1a"),
                String::from("V1b"),
                String::from("V2a"),
                String::from("V2b"),
                String::from("TAG S MEZERAMI"),
            ],
        };

        assert!(
            song.check_invariants()
                .is_err_and(|e| e.to_string().contains("obsahuje tag s mezerou"))
        )
    }

    #[test]
    fn check_invariants_test_matching_tags() {
        let song = Song {
            title: String::from("Haleluja (Svatý Pán Bůh Všemohoucí)"),
            author: None,
            parts: HashMap::from([
                (
                    String::from("C"),
                    String::from("Haleluja, haleluja,\nvládne nám všemocný Bůh a Král."),
                ),
                (
                    String::from("V1a"),
                    String::from(
                        "Haleluja, Svatý, Svatý,\nSvatý Pán Bůh Všemohoucí,\nhoden je On sám,\nBeránek, náš Pán,\npřijmout chválu,",
                    ),
                ),
                (
                    String::from("V1b"),
                    String::from(
                        "Svatý, Svatý Pán Bůh Všemohoucí,\nhoden je On sám,\nBeránek, náš Pán,\npřijmout chválu.",
                    ),
                ),
                (
                    String::from("V2a"),
                    String::from(
                        "Haleluja, Svatý, Svatý,\nTy jsi náš Bůh Všemohoucí,\npřijmi, Pane náš,\npřijmi, Pane náš,\nnaši chválu,",
                    ),
                ),
                (
                    String::from("V2b"),
                    String::from(
                        "Svatý, Ty jsi náš Bůh Všemohoucí,\npřijmi, Pane náš,\npřijmi, Pane náš,\nchválu.",
                    ),
                ),
            ]),
            order: vec![
                String::from("C"),
                String::from("V1a"),
                String::from("V1b"),
                String::from("V2a"),
                // String::from("V2b"), Chybí
            ],
        };

        assert!(
            song.check_invariants()
                .is_err_and(|e| e.to_string().contains("má odlišné tagy"))
        )
    }
}
