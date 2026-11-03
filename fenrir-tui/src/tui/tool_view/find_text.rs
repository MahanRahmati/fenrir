use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let pattern = params.get("pattern")?;
  if pattern.is_empty() {
    return None;
  }
  let label = match params.get("path") {
    Some(path) if path != "." => {
      format!("Search \"{pattern}\" in {path}")
    }
    _ => format!("Search \"{pattern}\""),
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result: false,
  });
}

pub fn result_label(view: &ToolCallView, result: &str) -> Option<String> {
  let count = if result.starts_with("No matches found") {
    0
  } else {
    result
      .lines()
      .map(str::trim)
      .filter(|line| !line.is_empty())
      .count()
  };
  let suffix = if count == 1 { "match" } else { "matches" };
  return Some(format!("{} ({count} {suffix})", view.label));
}
