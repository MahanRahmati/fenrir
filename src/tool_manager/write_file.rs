use std::fs;
use std::path::Path;

use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolParams};

pub struct WriteFileTool;

#[async_trait::async_trait]
impl Tool for WriteFileTool {
  fn name(&self) -> &str {
    return "write_file";
  }

  fn description(&self) -> &str {
    return "Write content to a file, creating it (including parent directories) if it does not exist. May use with edit_file for more complex edits.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "path": {"type": "string", "description": "Path of the file to write"},
        "content": {"type": "string", "description": "Content to write"},
      },
      "required": ["path", "content"],
    });
  }

  async fn execute(&self, params: ToolParams) -> ToolManagerResult<String> {
    let path = get_param::<String>(&params, "write_file", "path")?;
    let content = get_param::<String>(&params, "write_file", "content")?;

    let file_path = Path::new(&path);

    if let Some(parent) = file_path.parent() {
      fs::create_dir_all(parent).map_err(|e| {
        ToolManagerError::ExecutionFailed {
          name: "write_file".to_string(),
          reason: format!(
            "Cannot create parent directories for '{}': {}",
            &path, e
          ),
        }
      })?;
    }

    fs::write(file_path, &content).map_err(|e| {
      ToolManagerError::ExecutionFailed {
        name: "write_file".to_string(),
        reason: format!("Cannot write to file '{}': {}", &path, e),
      }
    })?;

    return Ok(format!(
      "Successfully wrote {} bytes to '{}'",
      content.len(),
      &path
    ));
  }
}
