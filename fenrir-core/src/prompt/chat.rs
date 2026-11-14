use crate::config::Config;
use crate::prompt::{PromptKind, PromptSpec, system};
use crate::provider_manager::Model;
use crate::session_manager::session::Session;

pub fn system_prompt(session: &Session, config: &Config) -> String {
  let mut prompt =
    system::build(config, session.persona_manager.current(), &session.cwd);
  if let Some(latest) = session.summaries.last() {
    prompt.push_str("\n\n");
    prompt.push_str(&latest.content);
  }
  return prompt;
}

pub fn build(session: &Session, model: &Model, system: &str) -> PromptSpec {
  let tools = session
    .tool_manager
    .get_tools_specs(session.persona_manager.current().allowed_tools());
  return PromptSpec {
    kind: PromptKind::Chat,
    model: model.clone(),
    system: system.to_string(),
    messages: session.build_chat_context(),
    tools,
    stream: true,
    emit_tokens: true,
  };
}
