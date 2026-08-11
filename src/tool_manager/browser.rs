use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::get_param;
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;

pub struct BrowserTool;

#[async_trait::async_trait]
impl Tool for BrowserTool {
  fn name(&self) -> &str {
    return "browser";
  }

  fn description(&self) -> &str {
    return "Control the web browser";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "action": {
          "type": "string",
          "description": "Action: open_url (open a URL in the browser)",
        },
        "url": {
          "type": "string",
          "description": "For open_url: the URL to open",
        },
      },
      "required": ["action"],
    });
  }

  fn is_read_only(&self) -> bool {
    return false;
  }

  async fn execute(&self, params: ToolParams) -> ToolManagerResult<String> {
    let action = get_param::<String>(&params, "browser", "action")?;

    match action.as_str() {
      "open_url" => {
        let url = get_param::<String>(&params, "browser", "url")?;

        let output = process::run("xdg-open", &[&url]).await.map_err(|e| {
          ToolManagerError::ExecutionFailed {
            name: "browser".to_string(),
            reason: format!("Failed to invoke xdg-open: {e}"),
          }
        })?;

        if !output.exit_code.is_success() {
          let stderr = output.stderr.trim();
          let details = if stderr.is_empty() {
            "the URL could not be opened".to_string()
          } else {
            stderr.to_string()
          };
          return Err(ToolManagerError::ExecutionFailed {
            name: "browser".to_string(),
            reason: details,
          });
        }

        return Ok(format!("Opened {url} in the default browser"));
      }
      _ => {
        return Err(ToolManagerError::ExecutionFailed {
          name: "browser".to_string(),
          reason: format!("Invalid action '{action}'. Valid actions: open_url"),
        });
      }
    }
  }
}
