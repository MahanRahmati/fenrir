INSERT INTO messages (
  id,
  session_id,
  role,
  content,
  timestamp,
  tool_call_id,
  tool_calls
) VALUES (?, ?, ?, ?, ?, ?, ?);
