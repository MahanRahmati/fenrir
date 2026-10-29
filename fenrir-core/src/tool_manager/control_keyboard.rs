use anyhow::{Context, Result, anyhow};

use crate::capability::keyboard;
use crate::tool_manager::helpers::get_param;
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};

pub struct ControlKeyboardTool;

#[async_trait::async_trait]
impl Tool for ControlKeyboardTool {
  fn name(&self) -> &str {
    return "control_keyboard";
  }

  fn description(&self) -> &str {
    return "Control the keyboard: press keys and type text.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "action": {
          "type": "string",
          "enum": ["type", "key"],
          "description": "What to do. 'type' types text, 'key' presses a key or key combination."
        },
        "text": {"type": "string", "description": "Text to type. Only used with action 'type'."},
        "key": {"type": "string", "description": "Key to press, e.g. 'super', 'enter', or a combination like 'super+d'. Only used with action 'key'."}
      },
      "required": ["action"],
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
    let action = get_param::<String>(&params, "control_keyboard", "action")?;
    return match action.as_str() {
      "type" => {
        let text = get_param::<String>(&params, "control_keyboard", "text")?;
        keyboard::type_with_keyboard(&text)
          .await
          .context(format!("Failed to type text '{text}' on keyboard"))
      }
      "key" => {
        let key = get_param::<String>(&params, "control_keyboard", "key")?;
        keyboard::press_key_on_keyboard(&key)
          .await
          .context(format!("Failed to press key '{key}' on keyboard"))
      }
      other => Err(anyhow!("Unknown action '{other}'")),
    };
  }
}
