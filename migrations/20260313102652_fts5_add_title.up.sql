DROP TRIGGER IF EXISTS entries_ai;
DROP TRIGGER IF EXISTS entries_ad;
DROP TRIGGER IF EXISTS entries_au;
DROP TABLE IF EXISTS entries_fts;

CREATE VIRTUAL TABLE entries_fts USING fts5(
    title,
    content_text,
    content=entries,
    content_rowid=id,
    tokenize='trigram'
);

CREATE TRIGGER entries_ai AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, title, content_text) VALUES (new.id, new.title, new.content_text);
END;

CREATE TRIGGER entries_ad AFTER DELETE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, title, content_text) VALUES('delete', old.id, old.title, old.content_text);
END;

CREATE TRIGGER entries_au AFTER UPDATE OF title, content_text ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, title, content_text) VALUES('delete', old.id, old.title, old.content_text);
    INSERT INTO entries_fts(rowid, title, content_text) VALUES (new.id, new.title, new.content_text);
END;

INSERT INTO entries_fts(entries_fts) VALUES('rebuild');
