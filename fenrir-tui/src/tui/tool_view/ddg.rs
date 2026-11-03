use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

const NO_RESULTS_PREFIX: &str = "No results found";

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let keyword = params.get("keyword")?.trim();
  if keyword.is_empty() {
    return None;
  }
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label: format!("Search \"{keyword}\""),
    body: Vec::new(),
    show_result: false,
  });
}

pub fn result_label(view: &ToolCallView, result: &str) -> Option<String> {
  let count = if result.starts_with(NO_RESULTS_PREFIX) {
    0
  } else {
    result
      .split("\n\n")
      .filter(|result| !result.trim().is_empty())
      .count()
  };
  let suffix = if count == 1 { "result" } else { "results" };
  return Some(format!("{} ({count} {suffix})", view.label));
}
