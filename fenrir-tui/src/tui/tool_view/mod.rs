mod read_file;

use fenrir_core::tool_manager::READ_FILE_TOOL_NAME;

const TOOL_CALL_ICON: &str = "●";

#[derive(Debug, PartialEq)]
pub struct ToolCallView {
  pub icon: &'static str,
  pub label: String,
  pub show_result: bool,
}

pub fn tool_call_view(name: &str, arguments: &str) -> Option<ToolCallView> {
  return match name {
    READ_FILE_TOOL_NAME => read_file::view(arguments),
    _ => None,
  };
}
