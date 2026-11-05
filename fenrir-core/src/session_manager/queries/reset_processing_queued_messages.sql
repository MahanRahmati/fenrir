UPDATE queued_messages
SET status = ?
WHERE session_id = ?
  AND status = ?;
