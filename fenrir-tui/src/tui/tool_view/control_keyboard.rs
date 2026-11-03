use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::task_summary;
use fenrir_core::tool_manager::tool::parse_tool_params;

const ACTION_TYPE: &str = "type";
const ACTION_KEY: &str = "key";

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let action = params.get("action")?.trim();
  let label = match action {
    ACTION_TYPE => {
      let text = task_summary(params.get("text")?);
      if text.is_empty() {
        return None;
      }
      format!("Type \"{text}\"")
    }
    ACTION_KEY => {
      let key = params.get("key")?.trim();
      if key.is_empty() {
        return None;
      }
      format!("Press {key}")
    }
    _ => return None,
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result: false,
  });
}
