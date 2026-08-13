use crate::capability::browser;
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::get_param;
use crate::tool_manager::tool::{Tool, ToolParams};

pub struct BrowserTool;

#[async_trait::async_trait]
impl Tool for BrowserTool {
  fn name(&self) -> &str {
    return "browser";
  }

  fn description(&self) -> &str {
    return "Control the web browser. Opens URLs in the default browser application, focuses its window, and saves the current page as a PDF";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "action": {
          "type": "string",
          "description": "Action: open_url (open a URL in the default browser and focus the browser window), save_as_pdf (save the current page as a PDF)",
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

    return match action.as_str() {
      "open_url" => {
        let url = get_param::<String>(&params, "browser", "url")?;
        browser::open_url(&url)
          .await
          .map_err(|e| tool_error(e.to_string()))
      }
      "save_as_pdf" => browser::save_as_pdf()
        .await
        .map(|destination| format!("Saved PDF to @{destination}"))
        .map_err(|e| tool_error(e.to_string())),
      _ => Err(tool_error(format!(
        "Invalid action '{action}'. Valid actions: open_url, save_as_pdf"
      ))),
    };
  }
}

fn tool_error(reason: impl Into<String>) -> ToolManagerError {
  return ToolManagerError::ExecutionFailed {
    name: "browser".to_string(),
    reason: reason.into(),
  };
}
