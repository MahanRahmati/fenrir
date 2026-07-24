use std::fs;

use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolParams};

pub struct RemoveFileTool;

impl Tool for RemoveFileTool {
  fn name(&self) -> &str {
    return "remove_file";
  }

  fn description(&self) -> &str {
    return "Remove a file from the filesystem. The file must exist.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "path": {"type": "string", "description": "Path of the file to remove"},
      },
      "required": ["path"],
    });
  }

  fn execute(&self, params: ToolParams) -> ToolManagerResult<String> {
    let path = get_param::<String>(&params, "remove_file", "path")?;

    fs::remove_file(&path).map_err(|e| ToolManagerError::ExecutionFailed {
      name: "remove_file".to_string(),
      reason: format!("Cannot remove file '{}': {}", &path, e),
    })?;

    return Ok(format!("Successfully removed '{}'", &path));
  }
}
