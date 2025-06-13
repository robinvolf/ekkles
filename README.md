# Ekkles

Rychlejší a modernější alternativa k [Opensongu](https://opensong.org/).

- Při vývoji je nejlepší na začátku použít `export DATABASE_URL='sqlite://db/database.sqlite3'` (bash) nebo `set -x DATABASE_URL=sqlite://db/database.sqlite3` (fish), pro nastavení URL vývojové databáze, závislost [sqlx](https://github.com/launchbadge/sqlx/tree/main?tab=readme-ov-file#compile-time-verification) používá makra pro verifikaci SQL dotazů při překladu (a skrze LSP i při vývoji v editoru)

## TODO

## Architektura

### GUI

- Musí to umět víc oken, jedno prezentované, druhé ovládací
- Framework [Iced](https://iced.rs/)

### Ukládání

- Chci mít něco, co umí *aspoň* importovat věci z Opensongu (nemusí to používat stejný formát a XML)
- Možná jednoduše SQLite databázi [Rusqlite](https://lib.rs/crates/rusqlite)

#### Bible

- Bude se to měnit? Asi moc ne, možná znovupoužít věci z Opensongu?
- [Bible jako API služba](https://bible.helloao.org/docs/guide/downloads.html)
- Možná eště lepší [Gighub Repo](https://github.com/Beblia/Holy-Bible-XML-Format/tree/master#)

### Plánované fičurky

- [ ] Možnost importu písní z programu OpenSong
- [ ] Rychlé a responzivní
  - Hledání písní
    - Fuzzy hledání?
  - Přepínání slajdů
- [ ] Promítání na nové okno, ovládané z původního
- [ ] Možnost editace písní, přidávání nových
- [ ] Možnost editace Programu (TODO název souboru písní)?
