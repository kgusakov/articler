-- Add up migration script here
-- Add CHECK constraints to ensure updated_at >= created_at

-- For users table
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL,
    email TEXT NOT NULL,
    name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL, -- Unix timestamp
    updated_at INTEGER NOT NULL, -- Unix timestamp
    CONSTRAINT unique_username UNIQUE (username),
    CONSTRAINT check_users_updated_at CHECK (updated_at >= created_at)
);

-- Fix invalid updated_at values and copy data
INSERT INTO users_new
SELECT
    id, username, email, name, password_hash, created_at,
    CASE WHEN updated_at < created_at THEN created_at ELSE updated_at END as updated_at
FROM users;

DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

-- For entries table
CREATE TABLE entries_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    hashed_url TEXT NOT NULL,
    given_url TEXT NOT NULL,
    hashed_given_url TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
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
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT check_entries_updated_at CHECK (updated_at >= created_at)
);

-- Fix invalid updated_at values and copy data
INSERT INTO entries_new
SELECT
    id, user_id, url, hashed_url, given_url, hashed_given_url, title, content,
    is_archived, archived_at, is_starred, starred_at, created_at,
    CASE WHEN updated_at < created_at THEN created_at ELSE updated_at END as updated_at,
    mimetype, language, reading_time, domain_name, preview_picture, origin_url,
    published_at, published_by, is_public, uid
FROM entries;

DROP TABLE entries;
ALTER TABLE entries_new RENAME TO entries;

-- For annotations table
CREATE TABLE annotations_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_id INTEGER NOT NULL,
    annotator_schema_version TEXT NOT NULL,
    text TEXT NOT NULL,
    created_at INTEGER NOT NULL, -- Unix timestamp
    updated_at INTEGER NOT NULL, -- Unix timestamp
    quote TEXT NOT NULL,
    FOREIGN KEY (entry_id) REFERENCES entries(id) ON DELETE CASCADE,
    CONSTRAINT check_annotations_updated_at CHECK (updated_at >= created_at)
);

-- Fix invalid updated_at values and copy data
INSERT INTO annotations_new
SELECT
    id, entry_id, annotator_schema_version, text, created_at,
    CASE WHEN updated_at < created_at THEN created_at ELSE updated_at END as updated_at,
    quote
FROM annotations;

DROP TABLE annotations;
ALTER TABLE annotations_new RENAME TO annotations;

-- Recreate the index for annotation_ranges that references annotations
-- Note: This index was created in the initial migration and needs to be preserved
CREATE INDEX IF NOT EXISTS idx_annotation_ranges_annotation_id ON annotation_ranges(annotation_id);
