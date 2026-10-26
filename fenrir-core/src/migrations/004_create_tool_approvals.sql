CREATE TABLE IF NOT EXISTS tool_approvals (
  tool_call_id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id),
  tool_name TEXT NOT NULL,
  message TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  decided_at INTEGER
);
