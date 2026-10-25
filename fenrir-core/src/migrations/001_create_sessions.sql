CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  parent_id TEXT,
  title TEXT,
  created_at INTEGER NOT NULL,
  last_interaction INTEGER NOT NULL
);
