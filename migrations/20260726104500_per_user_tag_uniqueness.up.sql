-- Add up migration script here
PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS tags_new (
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

DELETE FROM entry_tags
WHERE rowid IN (
    SELECT et.rowid
    FROM entry_tags et
    JOIN entries e ON e.id = et.entry_id
    JOIN tags t ON t.id = et.tag_id
    WHERE e.user_id <> t.user_id
);

PRAGMA foreign_keys = ON;
