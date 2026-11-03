use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

const ACTION_OPEN_URL: &str = "open_url";
const ACTION_SAVE_AS_PDF: &str = "save_as_pdf";
const SAVED_PREFIX: &str = "Saved PDF to @";

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let action = params.get("action")?.trim();
  let label = match action {
    ACTION_OPEN_URL => {
      let url = params.get("url")?.trim();
      if url.is_empty() {
        return None;
      }
      format!("Open {url}")
    }
    ACTION_SAVE_AS_PDF => String::from("Save page as PDF"),
    _ => return None,
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result: false,
  });
}

pub fn result_label(view: &ToolCallView, result: &str) -> Option<String> {
  let destination = result.trim().strip_prefix(SAVED_PREFIX)?;
  if destination.is_empty() {
    return None;
  }
  return Some(format!("{}: {destination}", view.label));
}
