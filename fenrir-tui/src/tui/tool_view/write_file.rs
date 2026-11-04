use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::ToolPreviewLine;
use fenrir_core::tool_manager::tool::parse_tool_params;

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
  let width = content.lines().count().to_string().len();
  return content
    .lines()
    .enumerate()
    .map(|(index, line)| {
      let number = index + 1;
      ToolPreviewLine::plain(format!("{number:>width$}. {line}"))
    })
    .collect();
}
