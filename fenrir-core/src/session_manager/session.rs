use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::ApprovalChannel;
use crate::config::Config;
use crate::hook_manager::HookManager;
use crate::persona_manager::PersonaManager;
use crate::prompt::Prompt;
use crate::provider_manager::{ChatUsage, Model};
use crate::session_manager::repository::{SessionRepository, SessionRow};
use crate::session_manager::summary::Summary;
use crate::tool_manager::ToolManager;
use crate::tool_manager::ToolPreview;
use crate::workflow_manager::WorkflowManager;

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
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub preview: Option<ToolPreview>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
  pub name: String,
  pub arguments: String,
}

pub struct Session {
  pub id: Uuid,
  pub parent_id: Option<Uuid>,
  pub title: Option<String>,
  pub created_at: i64,
  pub last_interaction: i64,
  pub messages: Vec<Message>,
  pub summaries: Vec<Summary>,
  pub model: Option<Model>,
  pub tool_manager: ToolManager,
  pub persona_manager: PersonaManager,
  pub workflow_manager: WorkflowManager,
  pub hook_manager: HookManager,
  approvals: ApprovalChannel,
  pub last_usage: Option<ChatUsage>,
  pub cwd: String,
}

impl Session {
  pub fn new(
    parent_id: Option<Uuid>,
    model: Option<Model>,
    config: &Config,
    approvals: ApprovalChannel,
    cwd: &str,
  ) -> Self {
    let now = Utc::now().timestamp();
    let row = SessionRow {
      id: Uuid::now_v7(),
      parent_id,
      title: None,
      created_at: now,
      last_interaction: now,
      messages: Vec::new(),
      summaries: Vec::new(),
    };
    return Self::from_row(row, config, model, approvals, cwd);
  }

  pub(crate) fn from_row(
    row: SessionRow,
    config: &Config,
    model: Option<Model>,
    approvals: ApprovalChannel,
    cwd: &str,
  ) -> Self {
    let is_subagent = row.parent_id.is_some();
    let tool_manager = ToolManager::new(config, is_subagent);
    let persona_manager = PersonaManager::new();
    let workflow_manager = WorkflowManager::new(config, cwd);
    let hook_manager = HookManager::new(config);
    return Self {
      id: row.id,
      parent_id: row.parent_id,
      title: row.title,
      created_at: row.created_at,
      last_interaction: row.last_interaction,
      messages: row.messages,
      summaries: row.summaries,
      model,
      tool_manager,
      persona_manager,
      workflow_manager,
      hook_manager,
      approvals,
      last_usage: None,
      cwd: cwd.to_owned(),
    };
  }

  pub fn is_subagent(&self) -> bool {
    return self.parent_id.is_some();
  }

  pub fn approval_channel(&self) -> &ApprovalChannel {
    return &self.approvals;
  }

  pub fn has_system_message(&self) -> bool {
    return self.messages.iter().any(|message| message.role == "system");
  }

  pub async fn ensure_system_message(
    &mut self,
    config: &Config,
    repository: &SessionRepository,
  ) -> Result<()> {
    if self.has_system_message() {
      return Ok(());
    }
    let prompt = Prompt::new(config, self.persona_manager.current(), &self.cwd);
    self.add_system_message(prompt.get_system_prompt());
    self.persist_last_message(repository).await?;
    return Ok(());
  }

  pub fn replace_system_message(&mut self, content: &str) {
    let Some(message) = self
      .messages
      .iter_mut()
      .find(|message| message.role == "system")
    else {
      return;
    };
    message.content = content.to_string();
  }

  pub async fn persist_last_message(
    &self,
    repository: &SessionRepository,
  ) -> Result<()> {
    let Some(message) = self.messages.last() else {
      return Ok(());
    };
    repository.insert_message(self.id, message).await?;
    return Ok(());
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

  pub fn add_tool_result_message(&mut self, tool_call_id: &str, content: &str) {
    self.add_tool_result(tool_call_id, &format!("Tool result: {content}"));
  }

  pub fn add_tool_call(
    &mut self,
    tool_call_id: &str,
    name: &str,
    arguments: &str,
    preview: Option<ToolPreview>,
  ) {
    let tool_calls = vec![ToolCall {
      id: tool_call_id.to_string(),
      call_type: "function".to_string(),
      function: ToolFunction {
        name: name.to_string(),
        arguments: arguments.to_string(),
      },
      preview,
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

      let mut system_content = self
        .messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone())
        .unwrap_or_default();
      if !system_content.is_empty() {
        system_content.push_str("\n\n");
      }
      system_content.push_str(&latest.content);

      chat_context.push(Message {
        id: Uuid::now_v7(),
        msg_type: None,
        role: "system".to_string(),
        content: system_content,
        tool_call_id: None,
        tool_calls: None,
        timestamp: latest.created_at,
      });

      let first_user_message = self.messages.iter().find(|m| m.role == "user");
      if let Some(first_user_message) = first_user_message {
        chat_context.push(first_user_message.clone());
      }

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
