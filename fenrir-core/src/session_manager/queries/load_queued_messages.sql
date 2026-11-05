SELECT * FROM queued_messages
WHERE session_id = ?
  AND status = ?
ORDER BY created_at ASC, id ASC;
