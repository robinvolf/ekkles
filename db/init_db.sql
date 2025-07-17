DROP TABLE IF EXISTS songs;
DROP TABLE IF EXISTS song_parts;
DROP TABLE IF EXISTS translations;
DROP TABLE IF EXISTS books;
DROP TABLE IF EXISTS verses;

CREATE TABLE songs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL UNIQUE,
    author TEXT,
    part_order TEXT NOT NULL -- Vektor uložený jako text, trochu hack
);

CREATE TABLE song_parts (
    song_id INTEGER NOT NULL,
    tag TEXT NOT NULL,
    lyrics TEXT NOT NULL,
    PRIMARY KEY (song_id, tag),
    FOREIGN KEY (song_id) REFERENCES songs (id) ON DELETE CASCADE -- Při smazání písně budou automaticky smazány všechny její části
);

CREATE TABLE translations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE books (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_order INTEGER NOT NULL UNIQUE, -- Pořadí knih v Bible (Genesis, Exodus, ... Zjevení)
    title TEXT NOT NULL UNIQUE
);

CREATE TABLE verses (
    translation_id INTEGER NOT NULL,
    book_id INTEGER NOT NULL,
    chapter INTEGER NOT NULL,
    number INTEGER NOT NULL,
    content TEXT NOT NULL,
    -- Pořadí veršů v daném překladu, abychom se mohli jednoduše dotazovat na rozsahy
    verse_order INTEGER NOT NULL,
    PRIMARY KEY (translation_id, book_id, chapter, number),
    FOREIGN KEY (book_id) REFERENCES books (id),
    FOREIGN KEY (translation_id) REFERENCES translations (id)
);

INSERT INTO books (book_order, title) VALUES
    (0, 'Genesis'),
    (1, 'Exodus'),
    (2, 'Leviticus'),
    (3, 'Numeri'),
    (4, 'Deuteronomium'),
    (5, 'Jozue'),
    (6, 'Soudců'),
    (7, 'Rút'),
    (8, '1. Samuelova'),
    (9, '2. Samuelova'),
    (10, '1. Královská'),
    (11, '2. Královská'),
    (12, '1. Paralipomenon'),
    (13, '2. Paralipomenon'),
    (14, 'Ezdráš'),
    (15, 'Nehemjáš'),
    (16, 'Ester'),
    (17, 'Jób'),
    (18, 'Žalmy'),
    (19, 'Přísloví'),
    (20, 'Kazatel'),
    (21, 'Píseň písní'),
    (22, 'Izajáš'),
    (23, 'Jeremjáš'),
    (24, 'Pláč'),
    (25, 'Ezechiel'),
    (26, 'Daniel'),
    (27, 'Ozeáš'),
    (28, 'Jóel'),
    (29, 'Ámos'),
    (30, 'Abdijáš'),
    (31, 'Jonáš'),
    (32, 'Micheáš'),
    (33, 'Nahum'),
    (34, 'Abakuk'),
    (35, 'Sofonjáš'),
    (36, 'Ageus'),
    (37, 'Zacharjáš'),
    (38, 'Malachiáš'),
    (39, 'Matouš'),
    (40, 'Marek'),
    (41, 'Lukáš'),
    (42, 'Jan'),
    (43, 'Skutky'),
    (44, 'Římanům'),
    (45, '1. Korintským'),
    (46, '2. Korintským'),
    (47, 'Galatským'),
    (48, 'Efezským'),
    (49, 'Filipským'),
    (50, 'Koloským'),
    (51, '1. Tesalonickým'),
    (52, '2. Tesalonickým'),
    (53, '1. Timoteovi'),
    (54, '2. Timoteovi'),
    (55, 'Titovi'),
    (56, 'Filemonovi'),
    (57, 'Židům'),
    (58, 'Jakub'),
    (59, '1. Petrova'),
    (60, '2. Petrova'),
    (61, '1. Janova'),
    (62, '2. Janova'),
    (63, '3. Janova'),
    (64, 'Juda'),
    (65, 'Zjevení')
