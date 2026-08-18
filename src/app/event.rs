use async_trait::async_trait;

pub enum AppEvent {
  Token { text: String, reasoning: bool },
  ToolCall { name: String, arguments: String },
  ToolResult { result: String, error: bool },
}

#[async_trait]
pub trait EventSink {
  fn on_event(&mut self, event: AppEvent);
  async fn confirm(&mut self, message: &str) -> bool;
}
