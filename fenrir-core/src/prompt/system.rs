use crate::config::Config;
use crate::persona_manager::Persona;
use crate::prompt::{Prompt, PromptKind};

pub fn build(
  config: &Config,
  persona: &Persona,
  cwd: &str,
  is_subagent: bool,
  kind: PromptKind,
) -> String {
  return Prompt::new(config, persona, cwd, is_subagent, kind)
    .get_system_prompt()
    .to_string();
}
