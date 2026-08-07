use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
  #[serde(rename = "type")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub msg_type: Option<String>,
  pub role: String,
  pub content: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tool_call_id: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tool_calls: Option<Vec<ToolCall>>,
  pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
  pub id: String,
  #[serde(rename = "type")]
  pub call_type: String,
  pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
  pub name: String,
  pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
  pub id: i64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  pub created_at: i64,
  pub last_interaction: i64,
  pub messages: Vec<Message>,
}

impl Session {
  pub fn new() -> Self {
    let now = Utc::now().timestamp();
    return Self {
      id: now,
      title: None,
      created_at: now,
      last_interaction: now,
      messages: Vec::new(),
    };
  }

  pub fn add_system_message(&mut self, content: &str) {
    self.add_message("system", content, None, None);
  }

  pub fn add_user_message(&mut self, content: &str) {
    self.add_message("user", content, None, None);
  }

  pub fn add_assistant_message(&mut self, content: &str) {
    self.add_message("assistant", content, None, None);
  }

  pub fn add_reasoning_message(&mut self, content: &str) {
    self.add_message("reasoning", content, None, None);
  }

  pub fn add_tool_result(&mut self, tool_call_id: &str, content: &str) {
    self.add_message("tool", content, Some(tool_call_id.to_string()), None);
  }

  pub fn add_tool_call(
    &mut self,
    tool_call_id: &str,
    name: &str,
    arguments: &str,
  ) {
    let tool_calls = vec![ToolCall {
      id: tool_call_id.to_string(),
      call_type: "function".to_string(),
      function: ToolFunction {
        name: name.to_string(),
        arguments: arguments.to_string(),
      },
    }];
    self.add_message(
      "assistant",
      "",
      Some(tool_call_id.to_string()),
      Some(tool_calls),
    );
  }

  fn add_message(
    &mut self,
    role: &str,
    content: &str,
    tool_call_id: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
  ) {
    let now = Utc::now().timestamp();
    let message = Message {
      msg_type: None,
      role: role.to_string(),
      content: content.to_string(),
      tool_call_id,
      tool_calls,
      timestamp: now,
    };
    self.last_interaction = now;
    self.messages.push(message);
  }
}
