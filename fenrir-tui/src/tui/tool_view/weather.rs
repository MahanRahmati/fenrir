use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::parse_tool_params;

const LOCATION_PREFIX: &str = "Weather for ";
const GENERIC_HEADER: &str = "Weather:";
const FEELS_LIKE_SUFFIX: &str = " (feels like";

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let location = params
    .get("location")
    .map(String::as_str)
    .map(str::trim)
    .filter(|location| !location.is_empty());
  let label = match location {
    Some(location) => format!("Weather in {location}"),
    None => String::from("Weather"),
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result: false,
  });
}

pub fn result_label(_view: &ToolCallView, result: &str) -> Option<String> {
  let mut lines = result.lines();
  let header = lines.next()?.trim();
  let conditions = lines.next()?.trim();
  if conditions.is_empty() {
    return None;
  }
  let conditions = conditions
    .split_once(FEELS_LIKE_SUFFIX)
    .map(|(conditions, _)| conditions)
    .unwrap_or(conditions);
  let area = if let Some(area) = header.strip_prefix(LOCATION_PREFIX) {
    area.strip_suffix(':')?.trim()
  } else if header == GENERIC_HEADER {
    return Some(format!("Weather: {conditions}"));
  } else {
    return None;
  };
  if area.is_empty() {
    return None;
  }
  return Some(format!("Weather for {area}: {conditions}"));
}
