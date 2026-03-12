-- Add up migration script here
CREATE VIRTUAL TABLE entries_fts USING fts5(
    content_text,
    content=entries,
    content_rowid=id,
    tokenize='trigram'
);

CREATE TRIGGER entries_ai AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, content_text) VALUES (new.id, new.content_text);
END;

CREATE TRIGGER entries_ad AFTER DELETE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, content_text) VALUES('delete', old.id, old.content_text);
END;

CREATE TRIGGER entries_au AFTER UPDATE OF content_text ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, content_text) VALUES('delete', old.id, old.content_text);
    INSERT INTO entries_fts(rowid, content_text) VALUES (new.id, new.content_text);
END;

INSERT INTO entries_fts(entries_fts) VALUES('rebuild');