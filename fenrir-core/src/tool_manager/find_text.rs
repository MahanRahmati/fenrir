use anyhow::{Context, Result, bail};

use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};
use crate::utils::process;

pub struct FindTextTool;

const DEFAULT_PATH: &str = ".";
const DEFAULT_MAX_RESULTS: usize = 50;
const DEFAULT_CASE_SENSITIVE: bool = true;

#[async_trait::async_trait]
impl Tool for FindTextTool {
  fn name(&self) -> &str {
    return "find_text";
  }

  fn description(&self) -> &str {
    return "Search file contents using ripgrep regex patterns. Supports line numbers and file glob filtering. Results are returned as newline-separated matches in the format 'path:line:column:text'.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "pattern": {"type": "string", "description": "Regex pattern to search for"},
        "path": {"type": "string", "description": "Directory or file to search in (default: current working directory)"},
        "file_glob": {"type": "string", "description": "Only search files matching this glob (e.g. *.rs, *.{cpp,js})"},
        "max_results": {"type": "integer", "description": "Maximum number of results to return (default: 50)"},
        "case_sensitive": {"type": "boolean", "description": "Whether to search case-sensitively (default: true)"},
      },
      "required": ["pattern"],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  async fn execute(
    &self,
    params: ToolParams,
    _context: &mut ToolContext<'_>,
  ) -> Result<String> {
    let pattern = get_param::<String>(&params, "find_text", "pattern")?;
    let path = get_optional_param::<String>(&params, "find_text", "path")
      .unwrap_or(DEFAULT_PATH.to_string());
    let max_results =
      get_optional_param::<usize>(&params, "find_text", "max_results")
        .unwrap_or(DEFAULT_MAX_RESULTS);
    let file_glob =
      get_optional_param::<String>(&params, "find_text", "file_glob");
    let case_sensitive =
      get_optional_param::<bool>(&params, "find_text", "case_sensitive")
        .unwrap_or(DEFAULT_CASE_SENSITIVE);

    let file_glob_ref = file_glob.as_deref();

    let mut args: Vec<&str> = vec![
      "--color",
      "never",
      "--with-filename",
      "--line-number",
      "--column",
    ];

    if !case_sensitive {
      args.push("-i");
    }

    if let Some(file_glob) = file_glob_ref {
      args.push("--glob");
      args.push(file_glob);
    }

    args.push(pattern.as_str());
    args.push(path.as_str());

    let output = process::run("rg", &args)
      .await
      .context("Failed to run rg")?;

    if output.exit_code.is_success() {
      let lines: Vec<&str> = output.stdout.lines().take(max_results).collect();
      if lines.is_empty() {
        return Ok(format!("No matches found for '{pattern}' in '{path}'."));
      }
      return Ok(lines.join("\n"));
    }

    if output.exit_code.is_no_match() {
      return Ok(format!("No matches found for '{pattern}' in '{path}'."));
    }

    bail!("ripgrep failed: {}", output.stderr.trim());
  }
}
