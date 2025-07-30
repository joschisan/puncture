CREATE TABLE recovery (
    id TEXT PRIMARY KEY NOT NULL,
    user_pk TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

ALTER TABLE user ADD COLUMN recovery_name TEXT; 