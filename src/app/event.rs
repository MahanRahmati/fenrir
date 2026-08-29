use async_trait::async_trait;
use uuid::Uuid;

use crate::app::errors::RuntimeResult;
use crate::persona_manager::Persona;
use crate::provider_manager::ChatUsage;
use crate::session_manager::session::Session;
use crate::slash_commands::SlashCommand;

pub enum AppEvent {
  UserMessage { content: String },
  Token { text: String, reasoning: bool },
  ToolCall { name: String, arguments: String },
  ToolResult { result: String, error: bool },
  Processing { processing: bool },
  Usage { usage: Option<ChatUsage> },
  Compacted { summary: String },
  Error { message: String },
}

#[async_trait]
pub trait EventSink {
  fn on_event(&mut self, event: AppEvent);
  fn confirm(&mut self, message: &str) -> bool;
  async fn read_input(&mut self) -> RuntimeResult<Option<String>>;
  fn on_exit(&mut self);
  fn on_help(&mut self, commands: Vec<SlashCommand>);
  fn on_sessions(
    &mut self,
    sessions: Vec<Session>,
    current: Option<Uuid>,
  ) -> RuntimeResult<Option<Uuid>>;
  fn on_session_selection(&mut self, session: Session);
  fn on_personas(
    &mut self,
    personas: Vec<Persona>,
    current: String,
  ) -> RuntimeResult<Option<String>>;
  fn on_persona_selection(&mut self, name: String);
}
