use std::path::Path;

use crate::files::operations::{create_directory_all, write_file};
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolParams};

pub const WRITE_FILE_TOOL_NAME: &str = "write_file";

pub struct WriteFileTool;

#[async_trait::async_trait]
impl Tool for WriteFileTool {
  fn name(&self) -> &str {
    return WRITE_FILE_TOOL_NAME;
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

  fn is_read_only(&self) -> bool {
    return false;
  }

  async fn execute(&self, params: ToolParams) -> ToolManagerResult<String> {
    let path = get_param::<String>(&params, WRITE_FILE_TOOL_NAME, "path")?;
    let content =
      get_param::<String>(&params, WRITE_FILE_TOOL_NAME, "content")?;

    if let Some(parent) = Path::new(&path).parent()
      && let Some(parent_str) = parent.as_os_str().to_str()
    {
      create_directory_all(parent_str).await.map_err(|e| {
        ToolManagerError::ExecutionFailed {
          name: WRITE_FILE_TOOL_NAME.to_string(),
          reason: format!(
            "Cannot create parent directories for '{}': {}",
            &path, e
          ),
        }
      })?;
    }

    write_file(&path, &content).await.map_err(|e| {
      ToolManagerError::ExecutionFailed {
        name: WRITE_FILE_TOOL_NAME.to_string(),
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
