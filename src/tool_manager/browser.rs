use crate::capability::window;
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::{get_optional_param, get_param};
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;

pub struct BrowserTool;

#[async_trait::async_trait]
impl Tool for BrowserTool {
  fn name(&self) -> &str {
    return "browser";
  }

  fn description(&self) -> &str {
    return "Control the web browser. Opens URLs in the default browser application and focuses its window";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "action": {
          "type": "string",
          "description": "Action: open_url (open a URL in the default browser and focus the browser window)",
        },
        "url": {
          "type": "string",
          "description": "For open_url: the URL to open",
        },
        "focus_delay": {
          "type": "string",
          "description": "For open_url: number of seconds to wait before focusing the browser window, e.g. '5'",
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
        let focus_delay =
          get_optional_param::<u64>(&params, "browser", "focus_delay");

        let window_ids = get_window_ids().await?;

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

        let focus_note =
          focus_new_browser_window(&window_ids, focus_delay).await?;

        return Ok(format!("Opened {url} in the default browser{focus_note}"));
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

async fn get_window_ids() -> ToolManagerResult<Vec<String>> {
  let window_ids = window::window_ids().await.map_err(|e| {
    ToolManagerError::ExecutionFailed {
      name: "browser".to_string(),
      reason: e.to_string(),
    }
  })?;
  return Ok(window_ids);
}

async fn focus_new_browser_window(
  before_ids: &[String],
  focus_delay: Option<u64>,
) -> ToolManagerResult<String> {
  match window::activate_window(before_ids, focus_delay).await {
    Ok(_) => match window::active_window().await {
      Ok(info) => Ok(format!(" and focused the browser window: {info}")),
      Err(_) => Ok(" and focused the browser window".to_string()),
    },
    Err(e) => Err(ToolManagerError::ExecutionFailed {
      name: "browser".to_string(),
      reason: format!("Browser window could not be focused: {e}"),
    }),
  }
}
