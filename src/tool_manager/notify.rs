use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::{get_optional_param, get_param};
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;

pub struct NotifyTool;

#[async_trait::async_trait]
impl Tool for NotifyTool {
  fn name(&self) -> &str {
    return "notify";
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

  async fn execute(&self, params: ToolParams) -> ToolManagerResult<String> {
    let summary = get_param::<String>(&params, "notify", "summary")?;
    let body = get_optional_param::<String>(&params, "notify", "body")
      .unwrap_or_default();

    let summary_arg = format!("string:{summary}");
    let body_arg = format!("string:{body}");

    let args: Vec<&str> = vec![
      "--session",
      "--dest=org.freedesktop.Notifications",
      "--type=method_call",
      "--print-reply",
      "/org/freedesktop/Notifications",
      "org.freedesktop.Notifications.Notify",
      "string:Fenrir",
      "uint32:0",
      "string:",
      &summary_arg,
      &body_arg,
      "array:string:",
      "dict:string:variant:",
      "int32:-1",
    ];

    let output = process::run("dbus-send", &args).await.map_err(|e| {
      ToolManagerError::ExecutionFailed {
        name: "notify".to_string(),
        reason: format!("Failed to invoke dbus-send: {e}"),
      }
    })?;

    if !output.exit_code.is_success() {
      let stderr = output.stderr.trim();
      let details = if stderr.is_empty() {
        "the notification could not be sent".to_string()
      } else {
        stderr.to_string()
      };
      return Err(ToolManagerError::ExecutionFailed {
        name: "notify".to_string(),
        reason: details,
      });
    }

    return match parse_notification_id(&output.stdout) {
      Some(id) => Ok(format!("Notification sent (id: {id})")),
      None => Ok("Notification sent".to_string()),
    };
  }
}

fn parse_notification_id(stdout: &str) -> Option<u32> {
  return stdout.lines().find_map(|line| {
    line
      .trim()
      .strip_prefix("uint32 ")
      .and_then(|v| v.parse().ok())
  });
}
