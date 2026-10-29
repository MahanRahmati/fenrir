use std::path::Path;
use std::time::Duration;

use crate::files::operations;
use crate::hook_manager::helpers::{self, tool_path};
use crate::hook_manager::hook::{HookContext, PostToolHook, ToolResultContext};
use crate::tool_manager::EDIT_FILE_TOOL_NAME;
use crate::tool_manager::WRITE_FILE_TOOL_NAME;
use crate::utils::path::is_path_within;

pub struct RustHook;

static STEPS: &[&[&str]] = &[&["fmt"], &["check"], &["clippy"]];

#[async_trait::async_trait]
impl PostToolHook for RustHook {
  fn name(&self) -> &str {
    return "rust";
  }

  async fn handle(
    &self,
    context: &HookContext<'_>,
    tool_result: &ToolResultContext<'_>,
  ) -> String {
    if *tool_result.error {
      return String::new();
    }

    if !matches!(tool_result.name, WRITE_FILE_TOOL_NAME | EDIT_FILE_TOOL_NAME) {
      return String::new();
    }

    let Some(path) = tool_path(tool_result.arguments) else {
      return String::new();
    };

    if Path::new(&path).extension().and_then(|e| e.to_str()) != Some("rs") {
      return String::new();
    }

    let cwd = match std::env::current_dir() {
      Ok(dir) => dir.canonicalize().unwrap_or(dir),
      Err(_) => return String::new(),
    };

    if !is_path_within(&path, &cwd) {
      return String::new();
    }

    if !operations::file_exists("Cargo.toml").await {
      return String::new();
    }

    let timeout =
      Duration::from_secs(context.config.hooks.hook_timeout_seconds);

    for &args in STEPS {
      let failure =
        helpers::run_command(self.name(), "cargo", args, timeout).await;

      if !failure.is_empty() {
        return failure;
      }
    }

    return String::new();
  }
}
