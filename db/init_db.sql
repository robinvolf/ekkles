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
    FOREIGN KEY (song_id) REFERENCES songs (id)
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
    PRIMARY KEY (translation_id, book_id, chapter, number),
    FOREIGN KEY (book_id) REFERENCES books (id),
    FOREIGN KEY (translation_id) REFERENCES translations (id)
);
