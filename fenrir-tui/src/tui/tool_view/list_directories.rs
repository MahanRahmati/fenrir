use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

const DEFAULT_PATH: &str = ".";

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let path = params
    .get("path")
    .map(String::as_str)
    .filter(|path| !path.is_empty())
    .unwrap_or(DEFAULT_PATH);
  let label = if path == DEFAULT_PATH || path.ends_with('/') {
    format!("List {path}")
  } else {
    format!("List {path}/")
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result: false,
  });
}

pub fn result_label(view: &ToolCallView, result: &str) -> Option<String> {
  let count = if result.starts_with("(empty directory)") {
    0
  } else {
    result
      .lines()
      .map(str::trim)
      .filter(|line| !line.is_empty())
      .count()
  };
  let suffix = if count == 1 { "entry" } else { "entries" };
  return Some(format!("{} ({count} {suffix})", view.label));
}
