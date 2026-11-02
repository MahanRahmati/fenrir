mod find_file;
mod find_text;
mod list_directories;
mod project_context;
mod read_file;

use fenrir_core::tool_manager::{
  FIND_FILE_TOOL_NAME, FIND_TEXT_TOOL_NAME, LIST_DIRECTORIES_TOOL_NAME,
  PROJECT_CONTEXT_TOOL_NAME, READ_FILE_TOOL_NAME,
};

const TOOL_CALL_ICON: &str = "●";

#[derive(Debug, PartialEq)]
pub struct ToolCallView {
  pub icon: &'static str,
  pub label: String,
  pub show_result: bool,
}

pub fn tool_call_view(name: &str, arguments: &str) -> Option<ToolCallView> {
  return match name {
    FIND_FILE_TOOL_NAME => find_file::view(arguments),
    FIND_TEXT_TOOL_NAME => find_text::view(arguments),
    LIST_DIRECTORIES_TOOL_NAME => list_directories::view(arguments),
    PROJECT_CONTEXT_TOOL_NAME => project_context::view(arguments),
    READ_FILE_TOOL_NAME => read_file::view(arguments),
    _ => None,
  };
}

pub fn tool_result_label(
  name: &str,
  view: &ToolCallView,
  result: &str,
) -> Option<String> {
  return match name {
    FIND_FILE_TOOL_NAME => find_file::result_label(view, result),
    FIND_TEXT_TOOL_NAME => find_text::result_label(view, result),
    LIST_DIRECTORIES_TOOL_NAME => list_directories::result_label(view, result),
    _ => None,
  };
}
