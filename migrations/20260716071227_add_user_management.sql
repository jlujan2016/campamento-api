-- Add migration script here
ALTER TABLE users ADD COLUMN is_blocked BOOLEAN NOT NULL DEFAULT false;
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);