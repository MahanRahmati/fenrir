use crate::capability::window;
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::get_param;
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;
use crate::utils::sleep::delay;

pub struct BrowserTool;

const DEFAULT_FOCUS_DELAY: u64 = 5;

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

        delay(Some(DEFAULT_FOCUS_DELAY)).await;
        let browser_id = get_browser_window_id().await?;
        let id =
          window::focus_window(&browser_id, None).await.map_err(|e| {
            ToolManagerError::ExecutionFailed {
              name: "browser".to_string(),
              reason: format!("Browser window could not be focused: {e}"),
            }
          })?;

        return Ok(format!("Opened {url} in the default browser{id}"));
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

async fn get_browser_window_id() -> ToolManagerResult<String> {
  let window_ids = get_window_ids().await?;
  if window_ids.is_empty() {
    return Err(ToolManagerError::ExecutionFailed {
      name: "browser".to_string(),
      reason: "No open windows found".to_string(),
    });
  }

  for window_id in &window_ids {
    let class = window::window_class(window_id).await;
    let Ok(class) = class else {
      continue;
    };
    if is_browser_class(&class) {
      return Ok(window_id.to_string());
    }
  }

  return Err(ToolManagerError::ExecutionFailed {
    name: "browser".to_string(),
    reason: "No open browser windows found".to_string(),
  });
}

fn is_browser_class(class: &str) -> bool {
  const BROWSERS: &[&str] = &[
    "helium",
    "chromium",
    "chrome",
    "brave",
    "firefox",
    "librewolf",
  ];
  return BROWSERS.iter().any(|browser| class.contains(browser));
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
