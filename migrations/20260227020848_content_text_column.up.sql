-- Add up migration script here
PRAGMA foreign_keys = OFF;

CREATE TABLE entries_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    hashed_url TEXT NOT NULL,
    given_url TEXT NOT NULL,
    hashed_given_url TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    content_text TEXT NOT NULL,
    is_archived BOOLEAN NOT NULL DEFAULT 0,
    archived_at INTEGER, -- Unix timestamp
    is_starred BOOLEAN NOT NULL DEFAULT 0,
    starred_at INTEGER, -- Unix timestamp
    created_at INTEGER NOT NULL, -- Unix timestamp
    updated_at INTEGER NOT NULL, -- Unix timestamp
    mimetype TEXT,
    language TEXT,
    reading_time INTEGER NOT NULL,
    domain_name TEXT NOT NULL,
    preview_picture TEXT,
    origin_url TEXT,
    published_at INTEGER, -- Unix timestamp
    published_by TEXT,
    is_public BOOLEAN,
    uid TEXT,
    CONSTRAINT check_entries_updated_at CHECK (updated_at >= created_at),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

INSERT INTO entries_new (
    id, user_id, url, hashed_url, given_url, hashed_given_url,
    title, content, content_text, is_archived, archived_at,
    is_starred, starred_at, created_at, updated_at, mimetype,
    language, reading_time, domain_name, preview_picture, origin_url,
    published_at, published_by, is_public, uid
)
SELECT
    id, user_id, url, hashed_url, given_url, hashed_given_url,
    title, content, '', is_archived, archived_at,
    is_starred, starred_at, created_at, updated_at, mimetype,
    language, reading_time, domain_name, preview_picture, origin_url,
    published_at, published_by, is_public, uid
FROM entries;

DROP TABLE entries;

ALTER TABLE entries_new RENAME TO entries;

PRAGMA foreign_keys = ON;