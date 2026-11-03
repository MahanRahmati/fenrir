use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let path = params.get("path")?;
  if path.is_empty() {
    return None;
  }
  let label = match (params.get("start_line"), params.get("end_line")) {
    (Some(start), Some(end)) => format!("Read {path} lines {start}-{end}"),
    (Some(start), None) => format!("Read {path} from line {start}"),
    (None, Some(end)) => format!("Read {path} through line {end}"),
    (None, None) => format!("Read {path}"),
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result: false,
  });
}
