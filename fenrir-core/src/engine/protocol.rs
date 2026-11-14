use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::command::Command;
use crate::persona_manager::Persona;
use crate::provider_manager::{ChatUsage, Model};
use crate::session_manager::{
  Message, QueuedMessage, Summary, ToolApprovalStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Correlation {
  pub session_id: Uuid,
  pub turn_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
  pub id: Uuid,
  pub parent_id: Option<Uuid>,
  pub title: Option<String>,
  pub created_at: i64,
  pub last_interaction: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionView {
  pub summary: SessionSummary,
  pub persona: String,
  pub model: Option<Model>,
  pub messages: Vec<Message>,
  pub summaries: Vec<Summary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnOutcome {
  Completed { assistant: String },
  Cancelled,
  Failed { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptMode {
  Normal,
  Review,
}

#[derive(Debug)]
pub enum BrokerRequest {
  Connect {
    cwd: String,
    events: mpsc::Sender<ServerEvent>,
    reply: oneshot::Sender<Uuid>,
  },
  Client {
    client_id: Uuid,
    request: ClientRequest,
  },
}

#[derive(Debug, Clone)]
pub enum ClientRequest {
  Disconnect,
  CreateSession {
    request_id: u64,
    persona: Option<String>,
    model: Option<Model>,
  },
  LoadSession {
    request_id: u64,
    session_id: Uuid,
  },
  ListSessions {
    request_id: u64,
    limit: Option<u32>,
  },
  ListPersonas {
    request_id: u64,
  },
  ListModels {
    request_id: u64,
    session_id: Option<Uuid>,
  },
  SetPersona {
    request_id: u64,
    session_id: Option<Uuid>,
    name: String,
  },
  SetModel {
    request_id: u64,
    session_id: Option<Uuid>,
    model: Model,
  },
  Subscribe {
    session_id: Uuid,
  },
  Unsubscribe {
    session_id: Uuid,
  },
  SendPrompt {
    request_id: u64,
    session_id: Option<Uuid>,
    mode: PromptMode,
    text: Option<String>,
  },
  RunCommand {
    request_id: u64,
    session_id: Option<Uuid>,
    command: Command,
  },
  Compact {
    request_id: u64,
    session_id: Uuid,
  },
  Predict {
    session_id: Uuid,
  },
  Cancel {
    session_id: Uuid,
    turn_id: Option<Uuid>,
  },
  ApproveTool {
    session_id: Uuid,
    turn_id: Uuid,
    tool_call_id: String,
    approved: bool,
  },
  Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerEvent {
  Connected {
    client_id: Uuid,
  },
  SessionCreated {
    request_id: u64,
    session: SessionView,
  },
  SessionLoaded {
    request_id: u64,
    session: SessionView,
  },
  SessionsListed {
    request_id: u64,
    sessions: Vec<SessionSummary>,
  },
  PersonasListed {
    request_id: u64,
    personas: Vec<PersonaInfo>,
    current: Option<String>,
  },
  ModelsListed {
    request_id: u64,
    models: Vec<Model>,
    current: Option<Model>,
  },
  PersonaUpdated {
    request_id: u64,
    session_id: Option<Uuid>,
    persona: String,
  },
  ModelUpdated {
    request_id: u64,
    session_id: Option<Uuid>,
    model: Model,
  },
  CommandCatalog {
    request_id: u64,
    commands: Vec<CommandInfo>,
  },
  UserMessage {
    correlation: Correlation,
    client_id: Uuid,
    content: String,
  },
  TurnStarted {
    correlation: Correlation,
    client_id: Uuid,
  },
  Token {
    correlation: Correlation,
    text: String,
    reasoning: bool,
  },
  ToolCall {
    correlation: Correlation,
    id: String,
    name: String,
    arguments: String,
    preview: Option<crate::tool_manager::ToolPreview>,
  },
  ToolApprovalRequest {
    correlation: Correlation,
    tool_call_id: String,
    tool_name: String,
    message: String,
  },
  ToolApprovalResult {
    correlation: Correlation,
    tool_call_id: String,
    status: ToolApprovalStatus,
  },
  ToolResult {
    correlation: Correlation,
    id: String,
    result: String,
    error: bool,
  },
  Usage {
    correlation: Correlation,
    usage: Option<ChatUsage>,
    context_window: Option<u64>,
  },
  Subagents {
    correlation: Correlation,
    delta: i32,
  },
  Retrying {
    correlation: Correlation,
    attempt: u32,
    max_attempts: u32,
    delay_seconds: u64,
    reason: String,
  },
  Compacted {
    session_id: Uuid,
    summary: String,
  },
  Prediction {
    session_id: Uuid,
    text: String,
  },
  TitleUpdated {
    session_id: Uuid,
    title: String,
  },
  QueueUpdated {
    session_id: Uuid,
    messages: Vec<QueuedMessage>,
  },
  TurnFinished {
    correlation: Correlation,
    outcome: TurnOutcome,
  },
  Error {
    request_id: Option<u64>,
    session_id: Option<Uuid>,
    message: String,
  },
  SessionStopped {
    session_id: Uuid,
  },
  Exited,
}

impl ServerEvent {
  pub fn name(&self) -> &'static str {
    return match self {
      ServerEvent::Connected { .. } => "connected",
      ServerEvent::SessionCreated { .. } => "session_created",
      ServerEvent::SessionLoaded { .. } => "session_loaded",
      ServerEvent::SessionsListed { .. } => "sessions_listed",
      ServerEvent::PersonasListed { .. } => "personas_listed",
      ServerEvent::ModelsListed { .. } => "models_listed",
      ServerEvent::PersonaUpdated { .. } => "persona_updated",
      ServerEvent::ModelUpdated { .. } => "model_updated",
      ServerEvent::CommandCatalog { .. } => "command_catalog",
      ServerEvent::UserMessage { .. } => "user_message",
      ServerEvent::TurnStarted { .. } => "turn_started",
      ServerEvent::Token { .. } => "token",
      ServerEvent::ToolCall { .. } => "tool_call",
      ServerEvent::ToolApprovalRequest { .. } => "tool_approval_request",
      ServerEvent::ToolApprovalResult { .. } => "tool_approval_result",
      ServerEvent::ToolResult { .. } => "tool_result",
      ServerEvent::Usage { .. } => "usage",
      ServerEvent::Subagents { .. } => "subagents",
      ServerEvent::Retrying { .. } => "retrying",
      ServerEvent::Compacted { .. } => "compacted",
      ServerEvent::Prediction { .. } => "prediction",
      ServerEvent::TitleUpdated { .. } => "title_updated",
      ServerEvent::QueueUpdated { .. } => "queue_updated",
      ServerEvent::TurnFinished { .. } => "turn_finished",
      ServerEvent::Error { .. } => "error",
      ServerEvent::SessionStopped { .. } => "session_stopped",
      ServerEvent::Exited => "exited",
    };
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInfo {
  pub name: String,
  pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaInfo {
  pub name: String,
  pub description: String,
  pub selectable: bool,
}

impl From<&Persona> for PersonaInfo {
  fn from(persona: &Persona) -> Self {
    return Self {
      name: persona.name().to_string(),
      description: persona.description().to_string(),
      selectable: persona.is_selectable(),
    };
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
  Approved,
  Denied,
}

#[derive(Debug)]
pub enum SessionEvent {
  Event(ServerEvent),
  ApprovalRequest {
    event: ServerEvent,
    reply: oneshot::Sender<ApprovalDecision>,
  },
  Stopped {
    session_id: Uuid,
    actor_id: Uuid,
  },
}
