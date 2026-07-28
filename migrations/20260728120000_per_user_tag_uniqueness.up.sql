-- no-transaction
PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TEMP TABLE tags_seq_backup AS
SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'tags'), 0) AS seq;

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

DELETE FROM entry_tags
WHERE NOT EXISTS (SELECT 1 FROM entries e WHERE e.id = entry_tags.entry_id)
   OR NOT EXISTS (SELECT 1 FROM tags    t WHERE t.id = entry_tags.tag_id)
   OR EXISTS (
        SELECT 1 FROM entries e JOIN tags t ON t.id = entry_tags.tag_id
        WHERE e.id = entry_tags.entry_id AND e.user_id <> t.user_id
      );

UPDATE sqlite_sequence
SET seq = (SELECT seq FROM tags_seq_backup)
WHERE name = 'tags' AND seq < (SELECT seq FROM tags_seq_backup);

DROP TABLE tags_seq_backup;

COMMIT;

PRAGMA foreign_keys = ON;
