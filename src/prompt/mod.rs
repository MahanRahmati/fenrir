use crate::utils::datetime::get_current_datetime;

const SYSTEM_PROMPT: &str = "You are Fenrir, a helpful AI assistant. Use tools when appropriate. Be concise and direct.";

pub struct Prompt {
  system_prompt: String,
}

impl Prompt {
  pub fn new() -> Self {
    let date_time_prompt = get_current_datetime();
    let full_prompt = format!("{}\n\n{}", SYSTEM_PROMPT, date_time_prompt,);
    return Self {
      system_prompt: full_prompt,
    };
  }

  pub fn get_system_prompt(&self) -> &str {
    return &self.system_prompt;
  }
}
