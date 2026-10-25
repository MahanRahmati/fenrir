use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use crate::agent_manager::agent::AgentKind;
use crate::app::AppEvent;
use crate::config::Config;
use crate::persona_manager::Persona;
use crate::tool_manager::tool::ToolParams;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
  Continue,
  Allow,
  Deny { reason: String, error: bool },
}

pub struct ToolCallContext<'a> {
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
  pub config: &'a Config,
  pub persona: &'a Persona,
  pub agent_kind: AgentKind,
  pub tx: &'a Sender<AppEvent>,
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
