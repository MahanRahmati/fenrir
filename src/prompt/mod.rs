use crate::config::Config;
use crate::utils::datetime::get_current_datetime;

const IDENTITY: &str = "You are Fenrir, a helpful AI assistant.";
const TOOLS: &str = "Use tools when appropriate.";
const EDIT: &str = "Before editing files, read them with `append_loc=true` for accurate line numbers. Prefer many small edits over one large one.";
const VERIFY: &str = "After making changes, use available tools to verify they work (e.g. `rust` tool for Rust projects if available).";
const STYLE: &str = "Be concise and direct.";

pub struct Prompt {
  system_prompt: String,
}

impl Prompt {
  pub fn new(config: Config) -> Self {
    let mut parts: Vec<&str> = Vec::new();
    parts.push(IDENTITY);
    parts.push(TOOLS);
    if config.permissions.read_file && config.permissions.edit_file {
      parts.push(EDIT);
    }
    parts.push(VERIFY);
    parts.push(STYLE);

    let system_prompt = parts.join(" ");
    let date_time_prompt = get_current_datetime();
    let full_prompt = format!("{}\n\n{}", system_prompt, date_time_prompt);
    return Self {
      system_prompt: full_prompt,
    };
  }

  pub fn get_system_prompt(&self) -> &str {
    return &self.system_prompt;
  }
}
