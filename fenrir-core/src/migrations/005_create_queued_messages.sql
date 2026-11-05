CREATE TABLE IF NOT EXISTS queued_messages (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id),
  content TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
