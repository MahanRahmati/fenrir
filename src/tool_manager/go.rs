use crate::files::operations;
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;

pub struct GoTool;

static STEPS: &[(&str, &[&str])] =
  &[("fmt", &["fmt"]), ("vet", &["vet"]), ("build", &["build"])];

#[async_trait::async_trait]
impl Tool for GoTool {
  fn name(&self) -> &str {
    "go"
  }

  fn description(&self) -> &str {
    "Run Go tooling: fmt (auto-fixes formatting), vet, build. Stops at the first failure."
  }

  fn parameters(&self) -> serde_json::Value {
    serde_json::json!({
      "type": "object",
      "properties": {},
      "required": [],
    })
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  async fn execute(&self, _params: ToolParams) -> ToolManagerResult<String> {
    if !operations::file_exists("go.mod").await {
      return Err(ToolManagerError::ExecutionFailed {
        name: "go".to_string(),
        reason:
          "This directory does not contain a Go project (no go.mod found)."
            .to_string(),
      });
    }

    for (name, args) in STEPS {
      let output = process::run("go", args).await.map_err(|e| {
        ToolManagerError::ExecutionFailed {
          name: "go".to_string(),
          reason: format!("Failed to run go {name}: {e}"),
        }
      })?;

      if !output.exit_code.is_success() {
        let stderr = output.stderr.trim();
        let stdout = output.stdout.trim();
        let details = if !stderr.is_empty() { stderr } else { stdout };
        return Err(ToolManagerError::ExecutionFailed {
          name: "go".to_string(),
          reason: format!("go {name} failed:\n{details}"),
        });
      }
    }

    return Ok("All checks passed (fmt, vet, build).".to_string());
  }
}
