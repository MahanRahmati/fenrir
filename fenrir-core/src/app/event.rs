use anyhow::Result;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::app::ApprovalChannel;
use crate::engine::protocol::{CommandInfo, SessionSummary};
use crate::persona_manager::Persona;
use crate::provider_manager::ChatUsage;
use crate::provider_manager::Model;
use crate::session_manager::ToolApprovalStatus;
use crate::session_manager::session::Session;
use crate::tool_manager::ToolPreview;

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
    preview: Option<ToolPreview>,
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
    context_window: Option<u64>,
  },
  Subagents {
    delta: i32,
  },
  Compacted {
    summary: String,
  },
  Prediction {
    text: String,
  },
  Error {
    message: String,
  },
  QueueUpdated {
    messages: Vec<String>,
  },
}

pub enum InputEvent {
  Pending,
  Input(String),
  Exit,
}

#[async_trait]
pub trait EventSink {
  async fn on_event(&mut self, event: AppEvent);
  async fn poll_input(&mut self) -> Result<InputEvent>;
  async fn poll_events(&mut self) -> Option<String>;
  fn wants_prediction(&self) -> bool {
    return false;
  }
  async fn on_exit(&mut self);
  async fn on_help(&mut self, commands: Vec<CommandInfo>);
  async fn on_sessions(
    &mut self,
    sessions: Vec<SessionSummary>,
    current: Option<Uuid>,
  ) -> Result<Option<Uuid>>;
  async fn on_session_selection(&mut self, session: &Session);
  async fn on_personas(
    &mut self,
    personas: Vec<Persona>,
    current: String,
  ) -> Result<Option<String>>;
  async fn on_persona_selection(&mut self, name: String);
  async fn on_new_session(
    &mut self,
    persona_name: String,
    model: Option<Model>,
  );
  async fn on_models(
    &mut self,
    models: Vec<Model>,
    current: Option<Model>,
  ) -> Result<Option<Model>>;
  async fn on_model_selection(&mut self, model: Model);
  async fn set_approval_channel(&mut self, channel: Option<ApprovalChannel>);
  async fn set_cancel_token(&mut self, token: CancellationToken);
}
