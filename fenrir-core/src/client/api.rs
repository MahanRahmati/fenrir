use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::protocol::{
  CommandInfo, PersonaInfo, PromptMode, SessionSummary, SessionView,
};
use crate::provider_manager::Model;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiStatus {
  Success,
  Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
  pub status: ApiStatus,
  pub data: Option<T>,
  pub message: Option<String>,
}

impl<T> ApiResponse<T> {
  pub fn success(data: T, message: impl Into<String>) -> Self {
    return Self {
      status: ApiStatus::Success,
      data: Some(data),
      message: Some(message.into()),
    };
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedData {
  pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionData {
  pub client_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsData {
  pub persona: String,
  pub model: Option<Model>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsData {
  pub sessions: Vec<SessionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
  pub session: SessionView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonasData {
  pub personas: Vec<PersonaInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsData {
  pub models: Vec<Model>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandsData {
  pub commands: Vec<CommandInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateConnectionRequest {
  pub cwd: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SubscribeRequest {
  pub request_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
  pub request_id: u64,
  pub persona: Option<String>,
  pub model: Option<Model>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptRequest {
  pub request_id: u64,
  pub session_id: Option<Uuid>,
  pub text: String,
  pub mode: Option<PromptMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaRequest {
  pub request_id: u64,
  pub session_id: Option<Uuid>,
  pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
  pub request_id: u64,
  pub session_id: Option<Uuid>,
  pub model: Model,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ReviewRequest {
  pub request_id: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CompactRequest {
  pub request_id: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct CancelRequest {
  pub turn_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
  pub turn_id: Uuid,
  pub tool_call_id: String,
  pub approved: bool,
}
