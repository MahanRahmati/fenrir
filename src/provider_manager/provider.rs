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

pub struct ChatResponse {
  pub content: Option<String>,
  pub tool_calls: Vec<ToolCall>,
}

pub trait Provider {
  fn name(&self) -> &str;
  fn active_model(&self) -> Option<&Model>;
  fn list_models(&self) -> Vec<Model>;
  fn chat(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> ProviderManagerResult<ChatResponse> {
    return Ok(ChatResponse {
      content: None,
      tool_calls: Vec::new(),
    });
  }
}
