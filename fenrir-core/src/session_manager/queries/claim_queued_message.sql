UPDATE queued_messages
SET status = ?
WHERE id = ?
  AND status = ?;
