use anyhow::{Context, Result, bail};

use std::path::Path;

use crate::files::operations;
use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolParams};

const DEFAULT_PATH: &str = ".";

pub struct ListDirectoriesTool;

#[async_trait::async_trait]
impl Tool for ListDirectoriesTool {
  fn name(&self) -> &str {
    return "list_directories";
  }

  fn description(&self) -> &str {
    return "List entries in a directory one level deep. Entries are returned one per line, with a trailing '/' for directories.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "path": {"type": "string", "description": "Directory to list (default: current working directory)"},
      },
      "required": [],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  async fn execute(&self, params: ToolParams) -> Result<String> {
    let path =
      get_optional_param::<String>(&params, "list_directories", "path")
        .unwrap_or(DEFAULT_PATH.to_string());

    let path = Path::new(&path);

    if !path.exists() {
      bail!("Path '{}' does not exist", path.display());
    }

    if !path.is_dir() {
      bail!("Path '{}' is not a directory", path.display());
    }

    let entries = operations::read_dir(&path.to_string_lossy())
      .await
      .context(format!("Failed to list directory '{}'", path.display()))?;

    if entries.is_empty() {
      return Ok("(empty directory)".to_string());
    }

    let results: Vec<String> = entries
      .into_iter()
      .map(
        |(name, is_dir)| {
          if is_dir { format!("{name}/") } else { name }
        },
      )
      .collect();

    return Ok(results.join("\n"));
  }
}
