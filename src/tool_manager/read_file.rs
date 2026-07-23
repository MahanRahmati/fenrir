use std::fs;

use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolParams};

pub struct ReadFileTool;

impl Tool for ReadFileTool {
  fn name(&self) -> &str {
    return "read_file";
  }

  fn description(&self) -> &str {
    return "Read the contents of a file. Optionally specify a 1-based line range. If append_loc is true, each line is prefixed with its line number (e.g. \"1\u{2192} ...\").";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "path": {"type": "string", "description": "Path to the file"},
        "start_line": {"type": "integer", "description": "First line to read, 1-based (default: 1)"},
        "end_line": {"type": "integer", "description": "Last line to read, 1-based inclusive (default: end of file)"},
        "append_loc": {"type": "boolean", "description": "Prefix each line with its line number"},
      },
      "required": ["path"],
    });
  }

  fn execute(&self, params: ToolParams) -> ToolManagerResult<String> {
    let path = get_param::<String>(&params, "read_file", "path")?;

    let content = fs::read_to_string(&path).map_err(|e| {
      ToolManagerError::ExecutionFailed {
        name: "read_file".to_string(),
        reason: format!("Cannot read file '{}': {}", path, e),
      }
    })?;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if total_lines == 0 {
      return Ok("".to_string());
    }

    let start_line: usize =
      get_optional_param::<usize>(&params, "read_file", "start_line")
        .unwrap_or(1)
        .saturating_sub(1);

    let end_line: Option<usize> =
      get_optional_param::<usize>(&params, "read_file", "end_line")
        .map(|v: usize| v.saturating_sub(1));

    let start = start_line.min(total_lines - 1);
    let end = end_line
      .map(|v| v.min(total_lines - 1))
      .unwrap_or(total_lines - 1);

    if start > end {
      return Err(ToolManagerError::ExecutionFailed {
        name: "read_file".to_string(),
        reason: format!(
          "start_line ({}) is greater than end_line ({})",
          start + 1,
          end + 1
        ),
      });
    }

    let selected_lines = &lines[start..=end];
    let append_loc: bool =
      get_optional_param::<bool>(&params, "read_file", "append_loc")
        .unwrap_or(false);

    let output: Vec<String> = if append_loc {
      selected_lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{}{} {}", start + i + 1, '\u{2192}', line))
        .collect()
    } else {
      selected_lines.iter().map(|&l| l.to_string()).collect()
    };

    return Ok(output.join("\n"));
  }
}
