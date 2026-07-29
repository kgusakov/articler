-- Add up migration script here
CREATE INDEX IF NOT EXISTS idx_clients_client_id ON clients(client_id);
