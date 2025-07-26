INSERT INTO songs (id, title, author, part_order) VALUES
    (0, 'Píseň1', 'Já :)', 'V1 C'),
    (1, 'Píseň2', NULL, 'V1 V2')
;

INSERT INTO song_parts (song_id, tag, lyrics) VALUES
    (0, 'V1', 'Text sloky V1'),
    (0, 'C', 'Text Refrénu C'),
    (1, 'V1', 'Text sloky V1'),
    (1, 'V2', 'Text sloky V2')
;

INSERT INTO translations (id, name) VALUES
    (0, 'Název překladu');

INSERT INTO verses (translation_id, book_id, chapter, number, content, verse_order) VALUES
    (0, 0, 1, 1, 'Verš 1',0),
    (0, 0, 1, 2, 'Verš 2',1),
    (0, 0, 1, 3, 'Verš 3',2),
    (0, 0, 1, 4, 'Verš 4',3),
    (0, 0, 1, 5, 'Verš 5',4),
    (0, 0, 1, 6, 'Verš 6',5),
    (0, 0, 1, 7, 'Verš 7',6),
    (0, 0, 1, 8, 'Verš 8',7),
    (0, 0, 1, 9, 'Verš 9',8),
    (0, 0, 1, 10, 'Verš 10',9);
