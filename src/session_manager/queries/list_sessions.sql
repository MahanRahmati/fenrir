SELECT * FROM sessions
WHERE parent_id IS NULL
ORDER BY last_interaction DESC;
