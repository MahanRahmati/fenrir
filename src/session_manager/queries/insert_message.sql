INSERT INTO messages (
  session_id,
  role,
  content,
  timestamp,
  tool_call_id,
  tool_calls
) VALUES (?, ?, ?, ?, ?, ?);
