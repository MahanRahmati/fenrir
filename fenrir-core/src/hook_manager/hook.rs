use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::engine::protocol::{Correlation, SessionEvent};
use crate::engine::repo_actor::RepoHandle;
use crate::persona_manager::Persona;
use crate::tool_manager::tool::ToolParams;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
  Continue,
  Allow,
  Deny { reason: String, error: bool },
}

pub struct ToolCallContext<'a> {
  pub id: &'a str,
  pub name: &'a str,
  pub params: &'a ToolParams,
  pub is_read_only: Option<bool>,
}

pub struct ToolResultContext<'a> {
  pub name: &'a str,
  pub arguments: &'a str,
  pub error: &'a bool,
}

pub struct HookContext<'a> {
  pub session_id: Uuid,
  pub correlation: Correlation,
  pub config: &'a Config,
  pub persona: &'a Persona,
  pub is_subagent: bool,
  pub events: &'a Sender<SessionEvent>,
  pub repository: &'a RepoHandle,
  pub cancel: &'a CancellationToken,
  pub cwd: &'a str,
}

#[async_trait::async_trait]
pub trait PreToolHook: Send + Sync {
  fn name(&self) -> &str;

  async fn handle(
    &self,
    context: &HookContext<'_>,
    tool_call: &ToolCallContext<'_>,
  ) -> HookDecision;
}

#[async_trait::async_trait]
pub trait PostToolHook: Send + Sync {
  fn name(&self) -> &str;

  async fn handle(
    &self,
    context: &HookContext<'_>,
    tool_result: &ToolResultContext<'_>,
  ) -> String;
}
