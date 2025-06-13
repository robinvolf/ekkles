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
