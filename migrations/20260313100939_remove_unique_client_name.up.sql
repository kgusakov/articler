-- Add up migration script here
PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS clients_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    client_id TEXT NOT NULL,
    client_secret TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL, -- Unix timestamp
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT unique_user_id_client_id UNIQUE (user_id, client_id)
);

INSERT INTO clients_new (
    id, name, client_id, client_secret, user_id, created_at
)
SELECT
    id, name, client_id, client_secret, user_id, created_at
FROM clients;

DROP TABLE clients;

ALTER TABLE clients_new RENAME TO clients;

PRAGMA foreign_keys = ON;