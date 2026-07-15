CREATE TABLE IF NOT EXISTS sessions (
  id INTEGER PRIMARY KEY,
  created_at INTEGER NOT NULL,
  last_interaction INTEGER NOT NULL
);
