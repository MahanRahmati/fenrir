use crate::files::operations::{read_to_string, write_file};
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolParams};

pub struct EditFileTool;

#[async_trait::async_trait]
impl Tool for EditFileTool {
  fn name(&self) -> &str {
    return "edit_file";
  }

  fn description(&self) -> &str {
    return "Edit a file by replacing a range of lines. ALWAYS first read the file with append_loc=true to get accurate line numbers. start_line and end_line are 1-based inclusive. If end_line exceeds the file length, new_content is appended after the last line. Use write_file to replace the whole file. Prefer many small edits over one large one.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "path": {"type": "string", "description": "Path to the file to edit"},
        "start_line": {"type": "integer", "description": "First line to replace, 1-based"},
        "end_line": {"type": "integer", "description": "Last line to replace, 1-based inclusive"},
        "new_content": {"type": "string", "description": "Content to replace the line range with"},
      },
      "required": ["path", "start_line", "end_line", "new_content"],
    });
  }

  fn is_read_only(&self) -> bool {
    return false;
  }

  async fn execute(&self, params: ToolParams) -> ToolManagerResult<String> {
    let path = get_param::<String>(&params, "edit_file", "path")?;
    let start_line = get_param::<usize>(&params, "edit_file", "start_line")?;
    let end_line = get_param::<usize>(&params, "edit_file", "end_line")?;
    let new_content = get_param::<String>(&params, "edit_file", "new_content")?;

    if start_line < 1 {
      return Err(ToolManagerError::ExecutionFailed {
        name: "edit_file".to_string(),
        reason: format!("start_line must be >= 1, got {}", start_line),
      });
    }
    if end_line < 1 {
      return Err(ToolManagerError::ExecutionFailed {
        name: "edit_file".to_string(),
        reason: format!("end_line must be >= 1, got {}", end_line),
      });
    }
    if start_line > end_line {
      return Err(ToolManagerError::ExecutionFailed {
        name: "edit_file".to_string(),
        reason: format!(
          "start_line ({}) must be <= end_line ({})",
          start_line, end_line
        ),
      });
    }
    if new_content.is_empty() {
      return Err(ToolManagerError::ExecutionFailed {
        name: "edit_file".to_string(),
        reason: "new_content must not be empty".to_string(),
      });
    }

    let content = read_to_string(&path).await.map_err(|e| {
      ToolManagerError::ExecutionFailed {
        name: "edit_file".to_string(),
        reason: format!("Cannot read file '{}': {}", path, e),
      }
    })?;

    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let total_lines = lines.len();

    let new_lines: Vec<String> =
      new_content.lines().map(|l| l.to_string()).collect();

    let (output, replaced) = if end_line > total_lines {
      let mut result = lines;
      result.extend(new_lines);
      (result.join("\n"), total_lines - start_line + 1)
    } else {
      let start_idx = start_line - 1;
      let end_idx = end_line.min(total_lines);
      let mut result: Vec<String> = Vec::with_capacity(
        total_lines - (end_idx - start_idx) + new_lines.len(),
      );
      result.extend_from_slice(&lines[..start_idx]);
      result.extend_from_slice(&new_lines);
      result.extend_from_slice(&lines[end_idx..]);
      (result.join("\n"), end_idx.saturating_sub(start_line) + 1)
    };

    write_file(&path, &output).await.map_err(|e| {
      ToolManagerError::ExecutionFailed {
        name: "edit_file".to_string(),
        reason: format!("Cannot write to file '{}': {}", path, e),
      }
    })?;

    return Ok(format!(
      "Successfully edited {} lines in '{}'",
      replaced, path
    ));
  }
}
