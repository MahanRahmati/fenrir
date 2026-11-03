use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::{task_summary, tool::parse_tool_params};

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let goal = params.get("goal")?;
  if goal.trim().is_empty() {
    return None;
  }
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label: format!("Check: {}", task_summary(goal)),
    body: Vec::new(),
    show_result: false,
  });
}
