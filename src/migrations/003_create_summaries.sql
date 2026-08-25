CREATE TABLE IF NOT EXISTS summaries (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id INTEGER NOT NULL REFERENCES sessions(id),
  content TEXT NOT NULL,
  covers_message_id INTEGER NOT NULL REFERENCES messages(id),
  created_at INTEGER NOT NULL
);
