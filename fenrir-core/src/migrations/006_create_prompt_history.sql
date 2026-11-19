CREATE TABLE IF NOT EXISTS prompt_history (
  id TEXT PRIMARY KEY,
  content TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
