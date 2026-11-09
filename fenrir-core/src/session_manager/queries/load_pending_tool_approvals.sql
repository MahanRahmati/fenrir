SELECT * FROM tool_approvals
WHERE session_id = ?
  AND status = 'pending'
ORDER BY created_at ASC, tool_call_id ASC;
