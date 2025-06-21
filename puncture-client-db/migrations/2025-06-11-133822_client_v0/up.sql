-- Your SQL goes here

CREATE TABLE daemon (
    node_id TEXT PRIMARY KEY,
    network TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL
); 