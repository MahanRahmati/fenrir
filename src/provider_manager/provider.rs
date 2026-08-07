use std::pin::Pin;

use futures::Stream;
use serde::Deserialize;

use crate::provider_manager::errors::ProviderManagerResult;
use crate::session_manager::Message;
use crate::tool_manager::tool::ToolSpec;

#[derive(Debug, Clone)]
pub struct Model {
  pub name: String,
  pub alias: Option<String>,
}

pub struct ToolCall {
  pub id: String,
  pub name: String,
  pub arguments: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ChatUsage {
  pub prompt_tokens: u64,
}

pub struct ChatResponse {
  pub content: Option<String>,
  pub reasoning: Option<String>,
  pub tool_calls: Vec<ToolCall>,
  pub usage: Option<ChatUsage>,
}

pub struct ChatStreamDelta {
  pub content: Option<String>,
  pub reasoning: Option<String>,
  pub tool_calls: Vec<StreamToolCallDelta>,
  pub usage: Option<ChatUsage>,
}

pub struct StreamToolCallDelta {
  pub index: usize,
  pub id: Option<String>,
  pub name: Option<String>,
  pub arguments: Option<String>,
}

pub type ChatStream =
  Pin<Box<dyn Stream<Item = ProviderManagerResult<ChatStreamDelta>> + Send>>;

#[async_trait::async_trait]
pub trait Provider {
  fn name(&self) -> &str;
  fn list_models(&self) -> Vec<Model>;
  fn set_active_model(&mut self, model: Model);
  async fn chat(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> ProviderManagerResult<ChatResponse>;
  async fn chat_stream(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> ProviderManagerResult<ChatStream>;
}
