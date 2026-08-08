use crate::capability::screenshot;

use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::get_optional_param;
use crate::tool_manager::tool::{Tool, ToolParams};

pub struct ScreenshotTool;

#[async_trait::async_trait]
impl Tool for ScreenshotTool {
  fn name(&self) -> &str {
    return "screenshot";
  }

  fn description(&self) -> &str {
    return "Take a full-screen screenshot and save it to a file. Returns the path of the saved screenshot.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "path": {"type": "string", "description": "Where to save the screenshot. Defaults to a file in the fenrir data directory."},
      },
      "required": [],
    });
  }

  fn is_read_only(&self) -> bool {
    return false;
  }

  async fn execute(&self, params: ToolParams) -> ToolManagerResult<String> {
    let path = get_optional_param::<String>(&params, "screenshot", "path");
    let destination = screenshot::screenshot(path).await.map_err(|e| {
      ToolManagerError::ExecutionFailed {
        name: "screenshot".to_string(),
        reason: e.to_string(),
      }
    })?;
    return Ok(format!("Screenshot saved to '{destination}'"));
  }
}
