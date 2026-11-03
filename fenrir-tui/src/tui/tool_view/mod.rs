mod browser;
mod control_keyboard;
mod date_time;
mod ddg;
mod find_file;
mod find_text;
mod goal_check;
mod list_directories;
mod notify;
mod project_context;
mod read_file;
mod read_pdf;
mod remove_file;
mod screenshot;
mod subagent;
mod todo_list;
mod weather;
mod write_file;

use fenrir_core::tool_manager::{
  BROWSER_TOOL_NAME, CONTROL_KEYBOARD_TOOL_NAME, DATE_TIME_TOOL_NAME,
  DDG_TOOL_NAME, FIND_FILE_TOOL_NAME, FIND_TEXT_TOOL_NAME,
  GOAL_CHECK_TOOL_NAME, LIST_DIRECTORIES_TOOL_NAME, NOTIFY_TOOL_NAME,
  PROJECT_CONTEXT_TOOL_NAME, READ_FILE_TOOL_NAME, READ_PDF_TOOL_NAME,
  REMOVE_FILE_TOOL_NAME, SCREENSHOT_TOOL_NAME, SUBAGENT_TOOL_NAME,
  TODO_LIST_TOOL_NAME, WEATHER_TOOL_NAME, WRITE_FILE_TOOL_NAME,
};

const TOOL_CALL_ICON: &str = "●";

#[derive(Debug, PartialEq)]
pub struct ToolCallView {
  pub icon: &'static str,
  pub label: String,
  pub body: Vec<String>,
  pub show_result: bool,
}

pub fn tool_call_view(name: &str, arguments: &str) -> Option<ToolCallView> {
  return match name {
    BROWSER_TOOL_NAME => browser::view(arguments),
    CONTROL_KEYBOARD_TOOL_NAME => control_keyboard::view(arguments),
    DATE_TIME_TOOL_NAME => date_time::view(arguments),
    DDG_TOOL_NAME => ddg::view(arguments),
    FIND_FILE_TOOL_NAME => find_file::view(arguments),
    FIND_TEXT_TOOL_NAME => find_text::view(arguments),
    GOAL_CHECK_TOOL_NAME => goal_check::view(arguments),
    LIST_DIRECTORIES_TOOL_NAME => list_directories::view(arguments),
    NOTIFY_TOOL_NAME => notify::view(arguments),
    PROJECT_CONTEXT_TOOL_NAME => project_context::view(arguments),
    READ_FILE_TOOL_NAME => read_file::view(arguments),
    READ_PDF_TOOL_NAME => read_pdf::view(arguments),
    REMOVE_FILE_TOOL_NAME => remove_file::view(arguments),
    SCREENSHOT_TOOL_NAME => screenshot::view(arguments),
    SUBAGENT_TOOL_NAME => subagent::view(arguments),
    TODO_LIST_TOOL_NAME => todo_list::view(arguments),
    WEATHER_TOOL_NAME => weather::view(arguments),
    WRITE_FILE_TOOL_NAME => write_file::view(arguments),
    _ => None,
  };
}

pub fn tool_result_label(
  name: &str,
  view: &ToolCallView,
  result: &str,
) -> Option<String> {
  return match name {
    BROWSER_TOOL_NAME => browser::result_label(view, result),
    DATE_TIME_TOOL_NAME => date_time::result_label(view, result),
    DDG_TOOL_NAME => ddg::result_label(view, result),
    FIND_FILE_TOOL_NAME => find_file::result_label(view, result),
    FIND_TEXT_TOOL_NAME => find_text::result_label(view, result),
    LIST_DIRECTORIES_TOOL_NAME => list_directories::result_label(view, result),
    READ_PDF_TOOL_NAME => read_pdf::result_label(view, result),
    SCREENSHOT_TOOL_NAME => screenshot::result_label(view, result),
    WEATHER_TOOL_NAME => weather::result_label(view, result),
    _ => None,
  };
}
