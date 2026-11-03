use super::{TOOL_CALL_ICON, ToolCallView};

const RESULT_PREFIX: &str = "Current date: ";

pub fn view(_arguments: &str) -> Option<ToolCallView> {
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label: String::from("Date and time"),
    body: Vec::new(),
    show_result: false,
  });
}

pub fn result_label(_view: &ToolCallView, result: &str) -> Option<String> {
  let datetime = result.trim().strip_prefix(RESULT_PREFIX)?;
  if datetime.is_empty() {
    return None;
  }
  return Some(format!("Date: {datetime}"));
}
