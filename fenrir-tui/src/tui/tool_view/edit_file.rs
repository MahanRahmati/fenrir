use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;
use fenrir_core::tool_manager::{ToolPreview, ToolPreviewLine};

pub fn view(
  arguments: &str,
  preview: Option<&ToolPreview>,
) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let path = params.get("path")?;
  let start_line = params.get("start_line")?.parse::<usize>().ok()?;
  let end_line = params.get("end_line")?.parse::<usize>().ok()?;
  let new_content = params.get("new_content")?;
  if path.is_empty() || new_content.is_empty() {
    return None;
  }
  let summary = preview
    .and_then(|preview| preview.summary.clone())
    .unwrap_or_else(|| fallback_summary(start_line, end_line));
  let body = match preview {
    Some(preview) => preview.lines.clone(),
    None => vec![ToolPreviewLine::warning("Preview unavailable")],
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label: format!("Edit {path}: {summary}"),
    body,
    show_result: false,
  });
}

fn fallback_summary(start_line: usize, end_line: usize) -> String {
  if start_line == end_line {
    return format!("line {start_line}");
  }
  return format!("lines {start_line}-{end_line}");
}
