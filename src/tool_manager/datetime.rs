use crate::tool_manager::ToolParams;
use crate::tool_manager::errors::ToolManagerResult;
use crate::tool_manager::tool::Tool;
use crate::utils::datetime::get_current_datetime;

pub struct DateTimeTool;

impl Tool for DateTimeTool {
  fn name(&self) -> &str {
    return "Date Time";
  }

  fn description(&self) -> &str {
    return "Returns the current local date and time.";
  }

  fn execute(&self, _params: ToolParams) -> ToolManagerResult<String> {
    return Ok(get_current_datetime());
  }
}
