use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

const PAGE_SEPARATOR: &str = " of ";

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let path = params.get("path")?.trim_start_matches('@');
  if path.is_empty() {
    return None;
  }
  let page = params.get("page")?.trim().parse::<u32>().ok()?;
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label: format!("Read {path} page {page}"),
    body: Vec::new(),
    show_result: false,
  });
}

pub fn result_label(view: &ToolCallView, result: &str) -> Option<String> {
  let total = result
    .trim()
    .strip_suffix(')')?
    .rsplit_once(PAGE_SEPARATOR)?
    .1
    .parse::<u32>()
    .ok()?;
  return Some(format!("{} of {total}", view.label));
}
