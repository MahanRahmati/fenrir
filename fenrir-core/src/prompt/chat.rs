use crate::config::Config;
use crate::prompt::{PromptKind, PromptSpec, system};
use crate::provider_manager::Model;
use crate::session_manager::session::Session;

pub fn build(session: &Session, config: &Config, model: &Model) -> PromptSpec {
  let tools = session
    .tool_manager
    .get_tools_specs(session.persona_manager.current().allowed_tools());
  let mut system =
    system::build(config, session.persona_manager.current(), &session.cwd);
  if let Some(latest) = session.summaries.last() {
    system.push_str("\n\n");
    system.push_str(&latest.content);
  }
  return PromptSpec {
    kind: PromptKind::Chat,
    model: model.clone(),
    system,
    messages: session.build_chat_context(),
    tools,
    stream: true,
    emit_tokens: true,
  };
}
