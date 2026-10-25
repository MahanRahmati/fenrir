use std::pin::Pin;

use anyhow::Result;
use futures::Stream;
use serde::Deserialize;

use crate::session_manager::Message;
use crate::tool_manager::tool::ToolSpec;

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
  pub provider: String,
  pub name: String,
  pub alias: Option<String>,
}

impl Model {
  pub fn label(&self) -> String {
    return match self.alias.clone() {
      Some(a) => a,
      _ => self.name.clone(),
    };
  }
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
  Pin<Box<dyn Stream<Item = Result<ChatStreamDelta>> + Send>>;

#[async_trait::async_trait]
pub trait Provider: Send + Sync {
  fn name(&self) -> &str;
  fn list_models(&self) -> Vec<Model>;
  async fn chat(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
    model: &str,
  ) -> Result<ChatResponse>;
  async fn chat_stream(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
    model: &str,
  ) -> Result<ChatStream>;
}
