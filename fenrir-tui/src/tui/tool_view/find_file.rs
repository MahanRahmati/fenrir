use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

const DEFAULT_INCLUDE: &str = "**";
const DEFAULT_PATH: &str = ".";

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let include = params
    .get("include")
    .map(String::as_str)
    .filter(|include| !include.is_empty() && *include != DEFAULT_INCLUDE);
  let path = params
    .get("path")
    .map(String::as_str)
    .filter(|path| *path != DEFAULT_PATH);
  let label = match (include, path) {
    (Some(include), Some(path)) => format!("Find {include} in {path}"),
    (Some(include), None) => format!("Find {include}"),
    (None, Some(path)) => format!("Find files in {path}"),
    (None, None) => "Find files".to_string(),
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result: false,
  });
}

pub fn result_label(view: &ToolCallView, result: &str) -> Option<String> {
  let count = if result.starts_with("No files found matching") {
    0
  } else {
    result
      .lines()
      .map(str::trim)
      .filter(|line| !line.is_empty())
      .count()
  };
  let suffix = if count == 1 { "file" } else { "files" };
  return Some(format!("{} ({count} {suffix})", view.label));
}
