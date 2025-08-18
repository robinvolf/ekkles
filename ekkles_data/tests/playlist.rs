// Jaké případy užití otestujeme:
//  - Vytvoření nového playlistu a jeho uložení (prázdný playlist)
//  - Vytvoření nového playlistu, modifikace (všeho druhu) a jeho uložení
//  - Načtení existujícího playlistu, jeho úprava a opětovné uložení
//
// TODO: - chce to další funkce pro songs, chcu umět hleda písně, aby to vracelo třá vektor (název, id)

mod common;
use ekkles_data::{
    Song,
    bible::{
        self, get_available_translations,
        indexing::{Book, VerseIndex},
    },
    playlist::{PlaylistMetadata, PlaylistMetadataStatus},
};
use pretty_assertions::assert_eq;

#[tokio::test]
async fn save_empty() {
    let pool = common::setup_db_with_bible_and_songs().await;

    let mut playlist = PlaylistMetadata::new("Testovací playlist");

    assert_eq!(playlist.get_status(), PlaylistMetadataStatus::Transient);

    playlist.save(pool.acquire().await.unwrap()).await.unwrap();

    assert!(matches!(
        playlist.get_status(),
        PlaylistMetadataStatus::Clean(_)
    ));
}

#[tokio::test]
async fn save_modified() {
    let pool = common::setup_db_with_bible_and_songs().await;

    let mut playlist = PlaylistMetadata::new("Testovací playlist");

    let song_id = Song::get_available_from_db(&pool)
        .await
        .unwrap()
        .first()
        .unwrap()
        .0;
    let translation_id = get_available_translations(&pool)
        .await
        .unwrap()
        .first()
        .unwrap()
        .0;

    playlist.push_song(song_id);
    playlist.push_bible_passage(
        translation_id,
        VerseIndex::try_new(Book::John, 1, 1).unwrap(),
        VerseIndex::try_new(Book::John, 1, 1).unwrap(),
    );

    assert_eq!(playlist.get_status(), PlaylistMetadataStatus::Transient);

    playlist.save(pool.acquire().await.unwrap()).await.unwrap();

    if let PlaylistMetadataStatus::Clean(id) = playlist.get_status() {
        let loaded_playlist = PlaylistMetadata::load(id, pool.acquire().await.unwrap())
            .await
            .unwrap();
        assert_eq!(loaded_playlist, playlist);
    } else {
        panic!();
    }
}
