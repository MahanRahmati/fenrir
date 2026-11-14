use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::task_summary;
use fenrir_core::tool_manager::tool::parse_tool_params;

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let task = params.get("task")?;
  if task.trim().is_empty() {
    return None;
  }
  let label = match params.get("complexity").map(String::as_str) {
    Some("hard") => format!("Subagent (hard) › {}", task_summary(task)),
    _ => format!("Subagent › {}", task_summary(task)),
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result: false,
  });
}
