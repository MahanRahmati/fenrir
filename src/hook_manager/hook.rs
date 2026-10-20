use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use crate::agent_manager::agent::AgentKind;
use crate::app::AppEvent;
use crate::config::Config;
use crate::hook_manager::errors::HookManagerResult;
use crate::persona_manager::Persona;
use crate::tool_manager::tool::ToolParams;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HookEvent {
  PreToolUse,
}

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

pub struct HookContext<'a> {
  pub session_id: Uuid,
  pub config: &'a Config,
  pub persona: &'a Persona,
  pub agent_kind: AgentKind,
  pub tx: &'a Sender<AppEvent>,
  pub tool_call: Option<ToolCallContext<'a>>,
}

#[async_trait::async_trait]
pub trait Hook: Send + Sync {
  fn name(&self) -> &str;
  fn events(&self) -> &[HookEvent];
  async fn handle(
    &self,
    event: HookEvent,
    context: &mut HookContext<'_>,
  ) -> HookManagerResult<HookDecision>;
}
