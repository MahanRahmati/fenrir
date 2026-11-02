use super::{TOOL_CALL_ICON, ToolCallView};

pub fn view(_arguments: &str) -> Option<ToolCallView> {
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label: String::from("Project context"),
    show_result: false,
  });
}
