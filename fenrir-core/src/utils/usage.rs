pub fn format_usage(tokens: u64) -> Option<String> {
  if tokens < 1000 {
    return Some(tokens.to_string());
  }
  return Some(format!("{:.1}K", tokens as f64 / 1000.0));
}
