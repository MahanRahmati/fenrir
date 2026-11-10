SELECT 1 FROM queued_messages
WHERE session_id = ?
LIMIT 1;
