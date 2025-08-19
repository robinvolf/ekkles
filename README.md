# Ekkles

Rychlejší a modernější alternativa k [Opensongu](https://opensong.org/).

## TODO

- [X] CLI utilitka pro import písní a biblí do SQLite databáze
  - Jediný problém tu budou async funkce, musí se tam dát tokio runtime
- [X] Začít pracovat na GUI Ekklesu, vůbec zjistit jak rozumně udělat víc oken/přechody mezi nimi
- [X] Datový model pro playlist, aby pak šel z GUI ukládat, načítat, editovat (CRUD)-[ ] Předělat v GUI kódu pokusy o zamknutí mutexu na `try_lock()` a kdyžtak tam hodit nějakou dummy hodnotu, ať neblokujeme GUI vlákno-[ ] Přidat možnost smazat playlist-[ ] Dodělat na backendu-[ ] Napsat testy pro backend-[ ] Zjistit, jestli nejdou nějak dobře psát testy pro Iced-[ ] Zprovozni přidávání písní-[ ] Napsat picker písní-[ ] Zprovozni přidávání pasáží-[ ] Napsat picker pasáží

## Vývoj

- Na začátku je dobré spustit v adresáři `db` příkaz `sqlite3 database.sqlite3 < init_db.sql`, aby se poté `sqlx` mohlo ptát databáze při kompilaci na schéma
  - Závislost [sqlx](https://github.com/launchbadge/sqlx/tree/main?tab=readme-ov-file#compile-time-verification) používá makra pro verifikaci SQL dotazů při překladu (a skrze LSP i při vývoji v editoru)

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
