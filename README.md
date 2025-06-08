# Ekkles

Rychlejší a modernější alternativa k [Opensongu](https://opensong.org/).

## TODO

- [ ] Zamyslet se, jestli zachovat nové řádky z načítané písně (zachování veršů jednotlivých slok). Může se to při promítání zdát podivné, že se jednotlivé verše nemapují na řádky.
- [ ] Omezit délku tagu, mohl by to teoreticky být enum?
- [ ] Validace při načítání písně (pokud je přítomné pořadí, kontrola, že všechny tagy v pořadí jsou přítomné i ve slovech)
- [ ] Zamyslet se a napsat, jak ukládat písně v databázi

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
