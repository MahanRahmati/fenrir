use crate::config::Config;
use crate::persona_manager::TITLES_PERSONA;
use crate::prompt::{PromptKind, PromptSpec, system};
use crate::provider_manager::Model;
use crate::session_manager::Message;

pub fn build(
  config: &Config,
  cwd: &str,
  model: &Model,
  first_message: Message,
) -> PromptSpec {
  return PromptSpec {
    kind: PromptKind::Title,
    model: model.clone(),
    system: system::build(config, &TITLES_PERSONA, cwd),
    messages: vec![first_message],
    tools: Vec::new(),
    stream: false,
    emit_tokens: false,
  };
}
