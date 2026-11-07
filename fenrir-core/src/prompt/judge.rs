pub fn build(goal: &str) -> String {
  return format!(
    "Determine whether this goal is fully achieved:\n\n{}",
    goal.trim()
  );
}
