pub fn format_usage(
  tokens: u64,
  context_window: Option<u64>,
) -> Option<String> {
  let usage = if tokens < 1000 {
    tokens.to_string()
  } else {
    format!("{:.1}K", tokens as f64 / 1000.0)
  };
  let Some(context_window) = context_window.filter(|window| *window > 0) else {
    return Some(usage);
  };
  let percentage = (tokens as f64 / context_window as f64 * 100.0).round();
  return Some(format!("{usage} ({percentage:.0}%)"));
}
