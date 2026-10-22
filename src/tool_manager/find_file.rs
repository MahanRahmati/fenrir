use anyhow::{Context, Result, bail};

use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;

pub struct FindFileTool;

const DEFAULT_INCLUDE: &str = "**";
const DEFAULT_MAX_RESULTS: usize = 50;

fn normalize_glob(pattern: &str) -> String {
  if !pattern.contains('/') {
    return format!("**/{pattern}");
  }
  if !pattern.starts_with("**/") && !pattern.starts_with('/') {
    return format!("**/{pattern}");
  }
  return pattern.to_string();
}

#[async_trait::async_trait]
impl Tool for FindFileTool {
  fn name(&self) -> &str {
    return "find_file";
  }

  fn description(&self) -> &str {
    return "Recursively search for files matching a glob pattern under a directory. Respects .gitignore and skips hidden directories automatically. A pattern with no '/' (e.g. \"*.rs\") matches the file's basename at any depth. A pattern containing '/' matches the full relative path; unless already anchored with \"**/\" or a leading '/', it is automatically prefixed with \"**/\".";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "path": {"type": "string", "description": "Base directory to search in"},
        "include": {"type": "string", "description": "Glob pattern for files to include (e.g. \"*.rs\" or \"src/**/*.rs\"). Default: **"},
        "exclude": {"type": "string", "description": "Glob pattern for files to exclude"},
        "max_results": {"type": "integer", "description": "Maximum number of results to return (default: 50)"},
      },
      "required": ["path"],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  async fn execute(&self, params: ToolParams) -> Result<String> {
    let path = get_param::<String>(&params, "find_file", "path")?;
    let include = get_optional_param::<String>(&params, "find_file", "include")
      .unwrap_or(DEFAULT_INCLUDE.to_string());
    let exclude = get_optional_param::<String>(&params, "find_file", "exclude");
    let max_results =
      get_optional_param::<usize>(&params, "find_file", "max_results")
        .unwrap_or(DEFAULT_MAX_RESULTS);

    let include_pattern = normalize_glob(&include);

    let max_results_str = max_results.to_string();
    let exclude_ref = exclude.as_deref();

    let mut args: Vec<&str> = vec![
      "--type",
      "f",
      "--glob",
      include_pattern.as_str(),
      "--base-directory",
      path.as_str(),
      "--max-results",
      max_results_str.as_str(),
    ];

    if let Some(exclude) = exclude_ref {
      args.push("--exclude");
      args.push(exclude);
    }

    let output = process::run("fd", &args)
      .await
      .context("Failed to run fd")?;

    if output.exit_code.is_success() {
      let results: Vec<&str> = output.stdout.lines().collect();
      if results.is_empty() {
        return Ok(format!("No files found matching '{include}' in '{path}'."));
      }
      return Ok(results.join("\n"));
    }

    if output.exit_code.is_no_match() {
      return Ok(format!("No files found matching '{include}' in '{path}'."));
    }

    bail!("fd failed: {}", output.stderr.trim());
  }
}
