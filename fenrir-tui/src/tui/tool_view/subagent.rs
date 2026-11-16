use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let task = params.get("task")?;
  if task.trim().is_empty() {
    return None;
  }
  let label = match params.get("complexity").map(String::as_str) {
    Some("hard") => "Subagent (hard)",
    _ => "Subagent",
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label: label.to_string(),
    body: Vec::new(),
    show_result: false,
  });
}
