use common::setup_db_with_bible;
use ekkles_data::bible::indexing::{Book, Passage, VerseIndex};
use ekkles_data::bible::parse_bible_from_xml;
use pretty_assertions::assert_eq;
use sqlx::query;
use tokio::fs::read_to_string;

mod common;

#[tokio::test]
async fn storing_bible() {
    let db = common::setup_bare_db().await;

    let xml_data = read_to_string("tests/data/CzechPrekladBible.xml")
        .await
        .unwrap();

    let res = parse_bible_from_xml(&xml_data, &db).await;

    assert!(res.is_ok());

    let expected_john = String::from(
        "„Neboť tak Bůh miluje svět, že dal svého jediného Syna, aby žádný, kdo v něho věří, nezahynul, ale měl život věčný.",
    );

    let expected_exodus = String::from(
        "Já Hospodin jsem tvůj Bůh, který jsem tě vyvedl z egyptské země, z domu otroctví.",
    );

    let book_id_john = query!("SELECT (id) FROM books WHERE title = $1", "Jan")
        .fetch_one(&db)
        .await
        .unwrap()
        .id
        .unwrap();

    let book_id_exodus = query!("SELECT (id) FROM books WHERE title = $1", "Exodus")
        .fetch_one(&db)
        .await
        .unwrap()
        .id
        .unwrap();

    let verse_content_john = query!(
        "SELECT (content) FROM verses WHERE book_id = $1 AND chapter = 3 AND number = 16",
        book_id_john
    )
    .fetch_one(&db)
    .await
    .unwrap()
    .content;

    let verse_content_exodus = query!(
        "SELECT (content) FROM verses WHERE book_id = $1 AND chapter = 20 AND number = 2",
        book_id_exodus
    )
    .fetch_one(&db)
    .await
    .unwrap()
    .content;

    assert_eq!(verse_content_john, expected_john);

    assert_eq!(verse_content_exodus, expected_exodus);
}

#[tokio::test]
async fn load_passage_one_book_test() {
    let db = setup_db_with_bible().await;

    let from = VerseIndex::try_new(Book::John, 21, 20).unwrap();
    let to = VerseIndex::try_new(Book::John, 21, 25).unwrap();

    let translation_id = query!("SELECT id FROM translations")
        .fetch_one(&db)
        .await
        .unwrap()
        .id;

    let expected_verses = [
        (
            20,
            String::from(
                "Petr se obrátil a uviděl, jak za nimi jde učedník, kterého Ježíš miloval a který se také při večeři naklonil k jeho prsům a řekl: ‚Pane, kdo je ten, který tě zrazuje?‘",
            ),
        ),
        (
            21,
            String::from("Když jej Petr uviděl, řekl Ježíšovi: „Pane, co bude s tímto?“"),
        ),
        (
            22,
            String::from(
                "Ježíš mu řekl: „Jestliže chci, aby tu zůstal, dokud nepřijdu, co je ti po tom? Ty mne následuj.“",
            ),
        ),
        (
            23,
            String::from(
                "A tak se mezi bratry rozšířilo to slovo, že onen učedník nezemře. Ježíš mu však neřekl, že nezemře, nýbrž: ‚Jestliže chci, aby tu zůstal, dokud nepřijdu, co je ti po tom?‘",
            ),
        ),
        (
            24,
            String::from(
                "To je ten učedník, který vydává svědectví o těchto věcech a který je zapsal; a víme, že jeho svědectví je pravdivé. ",
            ),
        ),
        (
            25,
            String::from(
                "Je ještě mnoho jiných věcí, které Ježíš učinil; kdyby se o každé zvlášť napsalo, myslím, že by celý svět neobsáhl knihy o tom napsané. Amen.",
            ),
        ),
    ];

    let passage = Passage::load(from, to, translation_id, &mut db.acquire().await.unwrap())
        .await
        .unwrap();

    let verses = passage.get_verses();

    assert_eq!(verses, expected_verses);
}

#[tokio::test]
async fn load_passage_over_book_boundary_test() {
    let db = setup_db_with_bible().await;

    let from = VerseIndex::try_new(Book::John, 21, 20).unwrap();
    let to = VerseIndex::try_new(Book::Acts, 1, 5).unwrap();

    let translation_id = query!("SELECT id FROM translations")
        .fetch_one(&db)
        .await
        .unwrap()
        .id;

    let expected_verses = [
        (
            20,
            String::from(
                "Petr se obrátil a uviděl, jak za nimi jde učedník, kterého Ježíš miloval a který se také při večeři naklonil k jeho prsům a řekl: ‚Pane, kdo je ten, který tě zrazuje?‘",
            ),
        ),
        (
            21,
            String::from("Když jej Petr uviděl, řekl Ježíšovi: „Pane, co bude s tímto?“"),
        ),
        (
            22,
            String::from(
                "Ježíš mu řekl: „Jestliže chci, aby tu zůstal, dokud nepřijdu, co je ti po tom? Ty mne následuj.“",
            ),
        ),
        (
            23,
            String::from(
                "A tak se mezi bratry rozšířilo to slovo, že onen učedník nezemře. Ježíš mu však neřekl, že nezemře, nýbrž: ‚Jestliže chci, aby tu zůstal, dokud nepřijdu, co je ti po tom?‘",
            ),
        ),
        (
            24,
            String::from(
                "To je ten učedník, který vydává svědectví o těchto věcech a který je zapsal; a víme, že jeho svědectví je pravdivé. ",
            ),
        ),
        (
            25,
            String::from(
                "Je ještě mnoho jiných věcí, které Ježíš učinil; kdyby se o každé zvlášť napsalo, myslím, že by celý svět neobsáhl knihy o tom napsané. Amen.",
            ),
        ),
        (
            1,
            String::from(
                "První zprávu, ó Theofile, jsem napsal o všem, co Ježíš začal činit a učit,",
            ),
        ),
        (
            2,
            String::from(
                "až do dne, kdy byl vzat vzhůru, když skrze Ducha Svatého dal příkazy apoštolům, které si vyvolil.",
            ),
        ),
        (
            3,
            String::from(
                "Jim také po svém utrpení mnoha důkazy prokázal, že žije, po čtyřicet dní se jim dával spatřit a říkal jim o Božím království.",
            ),
        ),
        (
            4,
            String::from(
                "A když s nimi jedl, nařídil jim, aby se nevzdalovali z Jeruzaléma, ale očekávali Otcovo zaslíbení – „které jste slyšeli ode mne,",
            ),
        ),
        (
            5,
            String::from(
                "neboť Jan křtil vodou, vy však po nemnohých těchto dnech budete pokřtěni v Duchu Svatém.“ ",
            ),
        ),
    ];

    let passage = Passage::load(from, to, translation_id, &mut db.acquire().await.unwrap())
        .await
        .unwrap();

    let verses = passage.get_verses();

    assert_eq!(verses, expected_verses);
}
