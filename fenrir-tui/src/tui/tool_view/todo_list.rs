use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

const SHOW_COMMAND: &str = "show";

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let todos = params.get("todos")?;
  let label = if todos.trim() == SHOW_COMMAND {
    String::from("Show todo list")
  } else {
    String::from("Update todo list")
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result: true,
  });
}
