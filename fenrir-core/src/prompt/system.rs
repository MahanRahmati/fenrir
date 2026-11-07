use crate::config::Config;
use crate::persona_manager::Persona;
use crate::prompt::Prompt;

pub fn build(config: &Config, persona: &Persona, cwd: &str) -> String {
  return Prompt::new(config, persona, cwd)
    .get_system_prompt()
    .to_string();
}
