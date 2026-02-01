use std::collections::HashMap;

use ekkles_data::Song;

mod common;

#[tokio::test]
async fn save_load_happy_path() {
    let pool = common::setup_bare_db().await;
    let mut conn = pool.acquire().await.unwrap();

    let mut song = Song {
        id: 0,
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
            String::from("V2b"),
        ],
    };

    let id = match song.save_new(&mut conn).await {
        Ok(id) => id,
        Err(e) => {
            println!("{:?}", e);
            panic!()
        }
    };

    song.id = id;

    match Song::load_from_db(id, &mut pool.acquire().await.unwrap()).await {
        Ok(loaded_song) => assert_eq!(loaded_song, song),
        Err(e) => {
            println!("{:?}", e);
            panic!()
        }
    }
}

#[tokio::test]
async fn save_corrupted_song() {
    let pool = common::setup_bare_db().await;
    let mut conn = pool.acquire().await.unwrap();

    let song = Song {
        id: 0,
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
            String::from("V2b"),
            String::from("Neexistující_tag"),
        ],
    };

    assert!(song.save_new(&mut conn).await.is_err());
}
