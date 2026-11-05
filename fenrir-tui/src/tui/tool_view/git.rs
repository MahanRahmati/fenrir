use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let subcommand = params.get("subcommand")?;
  if subcommand.is_empty() {
    return None;
  }
  let label = match params.get("path") {
    Some(path) if !path.is_empty() => {
      format!("Git {subcommand} {path}")
    }
    _ => format!("Git {subcommand}"),
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result: false,
  });
}
