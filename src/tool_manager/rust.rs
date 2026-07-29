use crate::files::operations;
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;

pub struct RustTool;

static STEPS: &[(&str, &[&str])] = &[
  ("fmt", &["fmt"]),
  ("check", &["check"]),
  ("clippy", &["clippy"]),
  ("test", &["test"]),
];

#[async_trait::async_trait]
impl Tool for RustTool {
  fn name(&self) -> &str {
    "rust"
  }

  fn description(&self) -> &str {
    "Run Rust tooling: fmt (auto-fixes formatting), check, clippy, test. Stops at the first failure."
  }

  fn parameters(&self) -> serde_json::Value {
    serde_json::json!({
      "type": "object",
      "properties": {},
      "required": [],
    })
  }

  async fn execute(&self, _params: ToolParams) -> ToolManagerResult<String> {
    if !operations::file_exists("Cargo.toml").await {
      return Err(ToolManagerError::ExecutionFailed {
        name: "rust".to_string(),
        reason: "This directory does not contain a Rust project (no Cargo.toml found)."
          .to_string(),
      });
    }

    for (name, args) in STEPS {
      let output = process::run("cargo", args).await.map_err(|e| {
        ToolManagerError::ExecutionFailed {
          name: "rust".to_string(),
          reason: format!("Failed to run cargo {}: {}", name, e),
        }
      })?;

      if !output.exit_code.is_success() {
        let stderr = output.stderr.trim();
        let stdout = output.stdout.trim();
        let details = if !stderr.is_empty() { stderr } else { stdout };
        return Err(ToolManagerError::ExecutionFailed {
          name: "rust".to_string(),
          reason: format!("cargo {} failed:\n{}", name, details),
        });
      }
    }

    return Ok("All checks passed (fmt, check, clippy, test).".to_string());
  }
}
