use crate::tool_manager::errors::ToolManagerResult;
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::datetime::get_current_datetime;

pub struct DateTimeTool;

#[async_trait::async_trait]
impl Tool for DateTimeTool {
  fn name(&self) -> &str {
    return "date_time";
  }

  fn description(&self) -> &str {
    return "Returns the current local date and time.";
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  async fn execute(&self, _params: ToolParams) -> ToolManagerResult<String> {
    return Ok(get_current_datetime());
  }
}
