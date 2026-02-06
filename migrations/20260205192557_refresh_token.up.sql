-- Add up migration script here
CREATE TABLE IF NOT EXISTS tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    token TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    client_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL, -- Unix timestamp
    expires_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
);

CREATE INDEX idx_expires ON tokens(expires_at);
CREATE INDEX idx_token ON tokens(token);