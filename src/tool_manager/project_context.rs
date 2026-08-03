use std::path::Path;

use crate::files::operations;
use crate::tool_manager::errors::ToolManagerResult;
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;

pub struct ProjectContextTool;

#[async_trait::async_trait]
impl Tool for ProjectContextTool {
  fn name(&self) -> &str {
    return "project_context";
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

  async fn execute(&self, _params: ToolParams) -> ToolManagerResult<String> {
    let mut sections: Vec<String> = Vec::new();

    sections.push(directory_section().await);

    if let Some(section) = git_section().await {
      sections.push(section);
    }

    if let Some(section) = agents_section().await {
      sections.push(section);
    }

    return Ok(sections.join("\n\n"));
  }
}

async fn directory_section() -> String {
  let mut listing = String::from("## Directory Structure\n");
  match operations::read_dir(".").await {
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
      listing.push_str(&format!("(error: {e})\n"));
    }
  }
  return listing;
}

async fn git_section() -> Option<String> {
  if !Path::new(".git").is_dir() {
    return None;
  }
  let output = process::run("git", &["branch", "--show-current"])
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

async fn agents_section() -> Option<String> {
  if !operations::file_exists("AGENTS.md").await {
    return None;
  }
  let content = operations::read_to_string("AGENTS.md").await.ok()?;
  return Some(format!("## AGENTS.md\n{}", content.trim()));
}
