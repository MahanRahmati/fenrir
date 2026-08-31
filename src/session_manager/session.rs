use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::session_manager::summary::Summary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
  pub id: Uuid,
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
  pub id: Uuid,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  pub created_at: i64,
  pub last_interaction: i64,
  pub messages: Vec<Message>,
  pub summaries: Vec<Summary>,
}

impl Session {
  pub fn new() -> Self {
    let now = Utc::now().timestamp();
    return Self {
      id: Uuid::now_v7(),
      title: None,
      created_at: now,
      last_interaction: now,
      messages: Vec::new(),
      summaries: Vec::new(),
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
      id: Uuid::now_v7(),
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

  pub fn add_summary(&mut self, content: &str, covers_message_id: Uuid) {
    let now = Utc::now().timestamp();
    let summary = Summary {
      id: Uuid::now_v7(),
      session_id: self.id,
      content: content.to_string(),
      covers_message_id,
      created_at: now,
    };
    self.last_interaction = now;
    self.summaries.push(summary);
  }

  pub fn build_chat_context(&self) -> Vec<Message> {
    if let Some(latest) = self.summaries.last() {
      let mut chat_context = Vec::new();

      let system_message = self.messages.iter().find(|m| m.role == "system");
      if let Some(system_message) = system_message {
        chat_context.push(system_message.clone());
      }

      let first_user_message = self.messages.iter().find(|m| m.role == "user");
      if let Some(first_user_message) = first_user_message {
        chat_context.push(first_user_message.clone());
      }

      let summary = Message {
        id: Uuid::now_v7(),
        msg_type: None,
        role: "system".to_string(),
        content: latest.content.clone(),
        tool_call_id: None,
        tool_calls: None,
        timestamp: latest.created_at,
      };
      chat_context.push(summary);

      for msg in &self.messages {
        if msg.id > latest.covers_message_id {
          chat_context.push(msg.clone());
        }
      }

      return chat_context;
    } else {
      return self.messages.clone();
    }
  }
}

impl Message {
  pub fn serialize_for_compaction(&self, max_chars: usize) -> String {
    return match self.role.as_str() {
      "system" => format!("[System]: {}", self.content),
      "user" => format!("[User]: {}", self.content),
      "assistant" => {
        if let Some(tool_calls) = &self.tool_calls {
          let calls: Vec<String> = tool_calls
            .iter()
            .map(|tc| {
              format!(
                "[Assistant tool call]: {}({})",
                tc.function.name, tc.function.arguments
              )
            })
            .collect();
          if !self.content.is_empty() {
            format!("[Assistant]: {}\n{}", self.content, calls.join("\n"))
          } else {
            calls.join("\n")
          }
        } else if !self.content.is_empty() {
          format!("[Assistant]: {}", self.content)
        } else {
          String::new()
        }
      }
      "tool" => {
        format!("[Tool result]: {}", truncate(&self.content, max_chars))
      }
      "reasoning" => format!("[Assistant reasoning]: {}", self.content),
      _ => format!("[{}]: {}", self.role, self.content),
    };
  }
}

fn truncate(content: &str, max_chars: usize) -> String {
  if content.len() <= max_chars {
    return content.to_string();
  }
  return format!("{}[truncated]", &content[..max_chars]);
}
