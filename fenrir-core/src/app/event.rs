use anyhow::Result;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::persona_manager::Persona;
use crate::provider_manager::ChatUsage;
use crate::provider_manager::Model;
use crate::session_manager::ToolApprovalStatus;
use crate::session_manager::session::Session;
use crate::slash_commands::SlashCommand;

pub enum AppEvent {
  UserMessage {
    content: String,
  },
  Token {
    text: String,
    reasoning: bool,
  },
  ToolCall {
    id: String,
    name: String,
    arguments: String,
  },
  ToolApprovalRequest {
    tool_call_id: String,
    tool_name: String,
    message: String,
  },
  ToolApprovalResult {
    tool_call_id: String,
    status: ToolApprovalStatus,
  },
  ToolResult {
    id: String,
    result: String,
    error: bool,
  },
  Processing {
    processing: bool,
  },
  Usage {
    usage: Option<ChatUsage>,
  },
  SubagentStarted {
    task: String,
  },
  Subagents {
    delta: i32,
  },
  Compacted {
    summary: String,
  },
  Error {
    message: String,
  },
}

#[async_trait]
pub trait EventSink {
  async fn on_event(&mut self, event: AppEvent);
  async fn read_input(&mut self) -> Result<Option<String>>;
  async fn poll_events(&mut self);
  fn on_exit(&mut self);
  fn on_help(&mut self, commands: Vec<SlashCommand>);
  fn on_sessions(
    &mut self,
    sessions: Vec<Session>,
    current: Option<Uuid>,
  ) -> Result<Option<Uuid>>;
  fn on_session_selection(&mut self, session: Session);
  fn on_personas(
    &mut self,
    personas: Vec<Persona>,
    current: String,
  ) -> Result<Option<String>>;
  fn on_persona_selection(&mut self, name: String);
  fn on_models(
    &mut self,
    models: Vec<Model>,
    current: Option<Model>,
  ) -> Result<Option<Model>>;
  fn on_model_selection(&mut self, model: Model);
  fn set_cancel_token(&mut self, token: CancellationToken);
}
