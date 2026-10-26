UPDATE tool_approvals
SET status = ?,
    decided_at = ?
WHERE tool_call_id = ?
AND status = 'pending';
