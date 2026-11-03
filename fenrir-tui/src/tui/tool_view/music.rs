use super::{TOOL_CALL_ICON, ToolCallView};
use fenrir_core::tool_manager::tool::{ToolParams, parse_tool_params};

pub fn view(arguments: &str) -> Option<ToolCallView> {
  let params = parse_tool_params(arguments);
  let action = param(&params, "action")?;
  let (label, show_result) = match action {
    "play" => {
      let label = match param(&params, "position") {
        Some(position) => format!("Play position {position}"),
        None => String::from("Play"),
      };
      (label, false)
    }
    "pause" => (String::from("Pause"), false),
    "next" => (String::from("Next track"), false),
    "previous" => (String::from("Previous track"), false),
    "volume" => {
      let label = match (param(&params, "value"), param(&params, "adjust")) {
        (Some(value), _) => format!("Set volume {value}"),
        (_, Some(adjust)) => format!("Adjust volume {adjust}"),
        _ => String::from("Volume"),
      };
      (label, false)
    }
    "add" => {
      let label = match param(&params, "file") {
        Some(file) => format!("Add {file}"),
        None => String::from("Add to queue"),
      };
      (label, false)
    }
    "clear" => (String::from("Clear queue"), false),
    "status" => (String::from("Show playback status"), true),
    "play_song" => {
      let query = param(&params, "query")?;
      (format!("Play song \"{query}\""), false)
    }
    _ => return None,
  };
  return Some(ToolCallView {
    icon: TOOL_CALL_ICON,
    label,
    body: Vec::new(),
    show_result,
  });
}

fn param<'a>(params: &'a ToolParams, name: &str) -> Option<&'a str> {
  return params
    .get(name)
    .map(String::as_str)
    .map(str::trim)
    .filter(|value| !value.is_empty());
}
