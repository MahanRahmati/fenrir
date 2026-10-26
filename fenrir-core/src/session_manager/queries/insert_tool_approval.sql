INSERT INTO tool_approvals (
  tool_call_id,
  session_id,
  tool_name,
  message,
  status,
  created_at,
  decided_at
) VALUES (?, ?, ?, ?, 'pending', ?, NULL);
