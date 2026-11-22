use std::fmt::Write;
use std::path::Path;

use anyhow::Result;

use crate::files::operations;
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};
use crate::utils::process;

pub const PROJECT_CONTEXT_TOOL_NAME: &str = "project_context";

pub struct ProjectContextTool;

#[async_trait::async_trait]
impl Tool for ProjectContextTool {
  fn name(&self) -> &str {
    return PROJECT_CONTEXT_TOOL_NAME;
  }

  fn description(&self) -> &str {
    return "Get an overview of the current project: directory listing and AGENTS.md.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {},
      "required": [],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  async fn execute(
    &self,
    _params: ToolParams,
    context: &mut ToolContext<'_>,
  ) -> Result<String> {
    let cwd = Path::new(context.cwd);
    let mut sections: Vec<String> = Vec::new();

    sections.push(directory_section(cwd).await);

    if let Some(section) = git_section(cwd).await {
      sections.push(section);
    }

    if let Some(section) = agents_section(cwd).await {
      sections.push(section);
    }

    return Ok(sections.join("\n\n"));
  }
}

async fn directory_section(cwd: &Path) -> String {
  let mut listing = String::from("## Directory Structure\n");
  match operations::read_dir(&cwd.to_string_lossy()).await {
    Ok(entries) => {
      for (name, is_dir) in &entries {
        listing.push_str(name);
        if *is_dir {
          listing.push_str("/\n");
        } else {
          listing.push('\n');
        }
      }
    }
    Err(e) => {
      let _ = writeln!(listing, "(error: {e})");
    }
  }
  return listing;
}

async fn git_section(cwd: &Path) -> Option<String> {
  if !cwd.join(".git").is_dir() {
    return None;
  }
  let output = process::run("git", &["branch", "--show-current"], Some(cwd))
    .await
    .ok()?;
  if !output.exit_code.is_success() {
    return None;
  }
  let s = output.stdout.trim().to_string();
  if s.is_empty() {
    return None;
  }
  return Some(format!("## Git Branch\n{s}"));
}

async fn agents_section(cwd: &Path) -> Option<String> {
  let path = cwd.join("AGENTS.md");
  let path = path.to_string_lossy();
  if !operations::file_exists(&path).await {
    return None;
  }
  let content = operations::read_to_string(&path).await.ok()?;
  return Some(format!("## AGENTS.md\n{}", content.trim()));
}
