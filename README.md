# Ekkles

Rychlejší a modernější alternativa k [Opensongu](https://opensong.org/).

## TODO

- [X] CLI utilitka pro import písní a biblí do SQLite databáze
  - Jediný problém tu budou async funkce, musí se tam dát tokio runtime
- [X] Začít pracovat na GUI Ekklesu, vůbec zjistit jak rozumně udělat víc oken/přechody mezi nimi
- [X] Datový model pro playlist, aby pak šel z GUI ukládat, načítat, editovat (CRUD)
- [X] Přidat možnost smazat playlist
  - [X] Dodělat na backendu
- [X] Napsat testy pro backend
- [X] Zjistit, jestli nejdou nějak dobře psát testy pro Iced
  - Eště nejdou, ve verzi 0.14 by měl přistát "test recorder", který by měl umožnit testovat GUI
- [X] Zprovozni přidávání písní
- [X] Napsat picker písní
- [X] Zprovozni přidávání pasáží
- [X] Napsat picker pasáží
- [ ] Předělat v GUI kódu pokusy o zamknutí mutexu na `try_lock()` a kdyžtak tam hodit nějakou dummy hodnotu, ať neblokujeme GUI vlákno
- [X] Prozkoumat možnost klávesových zkratek a přidat je na vhodná místa
  - [X] Prezentér (ovládání prezentovaného slajdu šipkama)
- [X] Zpřijemni manuální bible picker
  - Když vyberu knihu/kapitolu `from`, mělo by ji to nastavit i pro `to`, většinou vybírám verše ze stejné kapitoly
- [ ] Náhled pro výběr písní
- [ ] Náhled pro prezentér
- [ ] Možnost přidávání písní/veršů za běhu
- [ ] Ukončení prezentace by tě mělo hodit zpátky na editor
- [X] Přidat zamrznutí/začernění slajdu
- [ ] Přidat editor písní
- [ ] Rozhodnout, jak řešit vyhledání databáze, config
  - [ ] Rozhodnout co vůbec konfigurovat
- [ ] Prozkoumat modální okýnka (vanilla pomocí stack/overlay nebo nějaká [knihovnička](https://github.com/pml68/iced_dialog))
- [ ] Začleň ikonky pomocí custom fontů přes [iced_fonts](https://github.com/Redhawk18/iced_fonts)
- [ ] Implementuj drag-and-drop pro editor playlistů
  - Problémové, používám moc novou iced verzi (custom knihovničky nefungujou), pravděpodobně lepší počkat, než bude tato funkcionalita přímo v iced
- [ ] Lze optimalizovat některá místa, kde se mění obrazovka a místo klonování věcí lze použít [replace_with](https://docs.rs/replace_with/latest/replace_with/)
- [ ] Sniž závislost na mezi jednotlivými obrazovkami pomocí [senzorů](https://docs.iced.rs/iced/widget/sensor/struct.Sensor.html), které můžou při prvním načtení obrazovky začít načítat věci z databáze

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
