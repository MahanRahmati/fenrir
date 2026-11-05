INSERT INTO queued_messages (
  id,
  session_id,
  content,
  status,
  created_at
) VALUES (?, ?, ?, ?, ?);
