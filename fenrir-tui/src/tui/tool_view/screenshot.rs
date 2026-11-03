use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

const SAVED_PREFIX: &str = "Screenshot saved to ";

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let delay = params
    .get("delay")
    .map(String::as_str)
    .map(str::trim)
    .and_then(|delay| delay.parse::<u64>().ok())
    .filter(|delay| *delay > 0);
  let label = match delay {
    Some(delay) => format!("Screenshot in {delay}s"),
    None => String::from("Screenshot"),
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result: false,
  });
}

pub fn result_label(view: &ToolCallView, result: &str) -> Option<String> {
  let destination = result.trim().strip_prefix(SAVED_PREFIX)?;
  let destination = destination.strip_prefix('@').unwrap_or(destination);
  if destination.is_empty() {
    return None;
  }
  return Some(format!("{}: {destination}", view.label));
}
