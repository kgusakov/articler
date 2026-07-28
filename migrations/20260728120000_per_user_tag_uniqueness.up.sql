-- Add up migration script here
CREATE TABLE entry_tags_backup (entry_id INTEGER NOT NULL, tag_id INTEGER NOT NULL);

INSERT INTO entry_tags_backup (entry_id, tag_id)
SELECT entry_id, tag_id FROM entry_tags;

CREATE TABLE tags_seq_backup (seq INTEGER NOT NULL);

INSERT INTO tags_seq_backup (seq)
SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'tags'), 0);

CREATE TABLE tags_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    label TEXT NOT NULL,
    slug TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT unique_user_id_label UNIQUE (user_id, label),
    CONSTRAINT unique_user_id_slug UNIQUE (user_id, slug)
);

INSERT INTO tags_new (id, user_id, label, slug)
SELECT id, user_id, label, slug FROM tags;

DROP TABLE tags;

ALTER TABLE tags_new RENAME TO tags;

INSERT INTO entry_tags (entry_id, tag_id)
SELECT b.entry_id, b.tag_id
FROM entry_tags_backup b
JOIN entries e ON e.id = b.entry_id
JOIN tags t ON t.id = b.tag_id
WHERE e.user_id = t.user_id;

UPDATE sqlite_sequence
SET seq = (SELECT seq FROM tags_seq_backup)
WHERE name = 'tags' AND seq < (SELECT seq FROM tags_seq_backup);

DROP TABLE entry_tags_backup;

DROP TABLE tags_seq_backup;
