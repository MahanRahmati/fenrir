INSERT INTO sessions (
  id,
  parent_id,
  title,
  created_at,
  last_interaction,
  persona,
  model,
  cwd
) VALUES (?, ?, ?, ?, ?, ?, ?, ?);
