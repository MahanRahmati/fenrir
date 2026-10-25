CREATE TABLE IF NOT EXISTS summaries (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id),
  content TEXT NOT NULL,
  covers_message_id TEXT NOT NULL REFERENCES messages(id),
  created_at INTEGER NOT NULL
);
