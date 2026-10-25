use anyhow::{Context, Result};

use crate::files::operations::{remove_file, validate_file_exists};
use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolParams};

pub struct RemoveFileTool;

#[async_trait::async_trait]
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

  fn is_read_only(&self) -> bool {
    return false;
  }

  async fn execute(&self, params: ToolParams) -> Result<String> {
    let path = get_param::<String>(&params, "remove_file", "path")?;

    validate_file_exists(&path)
      .await
      .context(format!("File '{path}' does not exist"))?;

    remove_file(&path)
      .await
      .context(format!("Cannot remove file '{path}'"))?;

    return Ok(format!("Successfully removed '{}'", &path));
  }
}
