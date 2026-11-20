use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;
use fenrir_core::tool_manager::{ToolPreviewLine, ToolPreviewLineKind};

const EMPTY_FILE_BODY: &str = "(empty file)";

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let path = params.get("path")?;
  if path.is_empty() {
    return None;
  }
  let content = params.get("content")?;
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label: format!("Write {path}"),
    body: numbered_body(content),
    show_result: false,
  });
}

fn numbered_body(content: &str) -> Vec<ToolPreviewLine> {
  if content.is_empty() {
    return vec![ToolPreviewLine::plain(EMPTY_FILE_BODY)];
  }
  return content
    .lines()
    .enumerate()
    .map(|(index, line)| {
      ToolPreviewLine::numbered(ToolPreviewLineKind::Plain, index + 1, line)
    })
    .collect();
}
