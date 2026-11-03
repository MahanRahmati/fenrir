use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::task_summary;
use fenrir_core::tool_manager::tool::parse_tool_params;

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let summary = task_summary(params.get("summary")?);
  if summary.is_empty() {
    return None;
  }
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label: format!("Notify: {summary}"),
    body: Vec::new(),
    show_result: false,
  });
}
