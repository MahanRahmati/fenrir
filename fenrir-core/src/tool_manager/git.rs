use anyhow::{Context, Result, bail};

use crate::tool_manager::helpers::{get_optional_param, get_param};
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};
use crate::utils::process;

pub const GIT_TOOL_NAME: &str = "git";

const STATUS: &str = "status";
const DIFF: &str = "diff";
const LS_FILES: &str = "ls-files";

const CWD_FLAG: &str = "-C";
const PATHSPEC_SEPARATOR: &str = "--";
const STATUS_ARGS: &[&str] = &["status", "--porcelain"];
const DIFF_ARGS: &[&str] = &["diff", "--no-color", "--no-ext-diff"];
const LS_FILES_ARGS: &[&str] = &["ls-files"];

pub struct GitTool;

#[async_trait::async_trait]
impl Tool for GitTool {
  fn name(&self) -> &str {
    return GIT_TOOL_NAME;
  }

  fn description(&self) -> &str {
    return "Inspect the current git repository. Use 'status' for working tree status in porcelain format, 'diff' for unstaged changes, or 'ls-files' to list tracked files. Pass an optional path to limit the output.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "subcommand": {
          "type": "string",
          "enum": ["status", "diff", "ls-files"],
          "description": "Git subcommand: status (porcelain working tree status), diff (unstaged changes), ls-files (tracked files)"
        },
        "path": {
          "type": "string",
          "description": "Optional path or pathspec to limit the command to"
        }
      },
      "required": ["subcommand"],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  async fn execute(
    &self,
    params: ToolParams,
    context: &mut ToolContext<'_>,
  ) -> Result<String> {
    let subcommand = get_param::<String>(&params, GIT_TOOL_NAME, "subcommand")?;
    let path = get_optional_param::<String>(&params, GIT_TOOL_NAME, "path");
    let mut args = vec![CWD_FLAG, context.cwd];

    match subcommand.as_str() {
      STATUS => args.extend(STATUS_ARGS),
      DIFF => args.extend(DIFF_ARGS),
      LS_FILES => args.extend(LS_FILES_ARGS),
      _ => {
        bail!(
          "Invalid subcommand '{subcommand}'. Valid subcommands: status, diff, ls-files"
        );
      }
    }

    if let Some(path) = path.as_deref() {
      args.push(PATHSPEC_SEPARATOR);
      args.push(path);
    }

    let output = process::run("git", &args)
      .await
      .context("Failed to execute git")?;

    if !output.exit_code.is_success() {
      let stderr = output.stderr.trim();
      if stderr.is_empty() {
        bail!("git {subcommand} failed");
      }
      bail!("git {subcommand} failed: {stderr}");
    }

    let stdout = output.stdout.trim_end().to_string();
    if stdout.is_empty() {
      return Ok(empty_message(&subcommand).to_string());
    }
    return Ok(stdout);
  }
}

fn empty_message(subcommand: &str) -> &'static str {
  return match subcommand {
    STATUS => "Working tree clean.",
    DIFF => "No unstaged changes.",
    LS_FILES => "No tracked files.",
    _ => "No output.",
  };
}
