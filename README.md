# Ekkles

Rychlejší a modernější alternativa k [Opensongu](https://opensong.org/).

## TODO

- Rozmysli si, jestli má smysl udržovat status playlistu při jeho editaci (clean/transient/dirty). Ukládání do DB je přece velice levné, poté může být uložení immutable a nepotřebujeme mutex, což zjednoduší GUI kód
  - [ ] Předělat v GUI kódu pokusy o zamknutí mutexu na `try_lock()` a kdyžtak tam hodit nějakou dummy hodnotu, ať neblokujeme GUI vlákno
  - [ ] Ukončení prezentace by tě mělo hodit zpátky na editor
- [ ] Možnost přidávání písní/veršů za běhu
- [ ] Přidat editor písní
- [ ] Rozšiř konfiguraci (hardcoded věci), konfigurák, CLI, proměnné prostředí
- [ ] Začleň ikonky pomocí custom fontů přes [iced_fonts](https://github.com/Redhawk18/iced_fonts)
- [ ] Implementuj drag-and-drop pro editor playlistů
  - Problémové, používám moc novou iced verzi (custom knihovničky nefungujou), pravděpodobně lepší počkat, než bude tato funkcionalita přímo v iced
- [ ] Lze optimalizovat některá místa, kde se mění obrazovka a místo klonování věcí lze použít [replace_with](https://docs.rs/replace_with/latest/replace_with/)

## Bugísky
Žádné známé, hurá!

## Vývoj

- Na začátku je dobré spustit v adresáři `db` příkaz `sqlite3 database.sqlite3 < init_db.sql`, aby se poté `sqlx` mohlo ptát databáze při kompilaci na schéma
  - Závislost [sqlx](https://github.com/launchbadge/sqlx/tree/main?tab=readme-ov-file#compile-time-verification) používá makra pro verifikaci SQL dotazů při překladu (a skrze LSP i při vývoji v editoru)

## Architektura

### GUI

- Framework [Iced](https://iced.rs/)
- Celá obrazovka je rozdělena na jednotlivé `Screen`, jejichž detaily jsou implementované v jednotlivých modulech v `src/`
- Funkce pro `update` a `view` jednotlivých obrazovek jsou implementována v jejich modulech a volána z centrální `view` a `update` (`main.rs` nebo `update.rs`)

### Ukládání

- Všechny ne-konfigurační data jsou uloženy v SQLite databázi, schéma viz `ekkles_data/db/init_db.sql`

#### Bible

- Formát biblí z tohoto [repozitáře](https://github.com/Beblia/Holy-Bible-XML-Format/tree/master#)
- Proč?
  - Nemění se, není potřeba updatovat
  - Toto je dané volně k dispozici
  - Mají k dispozici hrozně moc překladů v různých jazycích
