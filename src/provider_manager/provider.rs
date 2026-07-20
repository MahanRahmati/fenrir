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
}
