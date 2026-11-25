CREATE TABLE revoked_sessions (
 token_hash TEXT PRIMARY KEY,
 expires_at INTEGER NOT NULL
);
ALTER TABLE purchasing_reviews ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
