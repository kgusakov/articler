-- Add up migration script here
CREATE TABLE IF NOT EXISTS entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL,
    hashed_url TEXT,
    given_url TEXT,
    hashed_given_url TEXT,
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
    uid TEXT
);

CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT NOT NULL UNIQUE,
    slug TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS entry_tags (
    entry_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (entry_id, tag_id),
    FOREIGN KEY (entry_id) REFERENCES entries(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS annotations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_id INTEGER NOT NULL,
    annotator_schema_version TEXT NOT NULL,
    text TEXT NOT NULL,
    created_at INTEGER NOT NULL, -- Unix timestamp
    updated_at INTEGER NOT NULL, -- Unix timestamp
    quote TEXT NOT NULL,
    FOREIGN KEY (entry_id) REFERENCES entries(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS annotation_ranges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    annotation_id INTEGER NOT NULL,
    start TEXT NOT NULL,
    end TEXT NOT NULL,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    FOREIGN KEY (annotation_id) REFERENCES annotations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_annotation_ranges_annotation_id ON annotation_ranges(annotation_id);