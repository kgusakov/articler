-- Add up migration script here
CREATE INDEX IF NOT EXISTS idx_entries_user_created ON entries(user_id, created_at);
