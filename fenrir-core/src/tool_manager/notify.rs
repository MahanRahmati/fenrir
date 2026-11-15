use anyhow::{Context, Result};

use crate::capability::notify;
use crate::tool_manager::helpers::{get_optional_param, get_param};
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};

pub const NOTIFY_TOOL_NAME: &str = "notify";

pub struct NotifyTool;

#[async_trait::async_trait]
impl Tool for NotifyTool {
  fn name(&self) -> &str {
    return NOTIFY_TOOL_NAME;
  }

  fn description(&self) -> &str {
    return "Send a desktop notification to the user.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "summary": {"type": "string", "description": "Title of the notification"},
        "body": {"type": "string", "description": "Body text of the notification"},
      },
      "required": ["summary"],
    });
  }

  fn is_read_only(&self) -> bool {
    return false;
  }

  async fn execute(
    &self,
    params: ToolParams,
    _context: &mut ToolContext<'_>,
  ) -> Result<String> {
    let summary = get_param::<String>(&params, NOTIFY_TOOL_NAME, "summary")?;
    let body = get_optional_param::<String>(&params, NOTIFY_TOOL_NAME, "body")
      .unwrap_or_default();

    return notify::send_notification(&summary, &body)
      .await
      .context("Failed to send notification");
  }
}
