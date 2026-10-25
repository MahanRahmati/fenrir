use crate::config::Config;
use crate::persona_manager::Persona;
use crate::utils::datetime::get_current_datetime;

const TOOLS: &str = "Use tools when appropriate.";
const EDIT: &str = "Before editing files, read them with `append_loc=true` for accurate line numbers. Prefer many small edits over one large one.";
const VERIFY: &str = "After making changes, verify they work.";
const CWD_RESTRICTED: &str = "Tools are restricted to the current working directory. Always use paths within the current directory; paths outside the current working directory are blocked.";
const CONCISE: &str = "Be concise and direct.";
const PATHS: &str = "Show file paths clearly when working with files.";
const MENTIONS: &str = "Files mentioned with @path are automatically attached to the message (as text, or as an image). Never use read_file on an attached file.";

pub struct Prompt {
  system_prompt: String,
}

impl Prompt {
  pub fn new(config: &Config, persona: &Persona) -> Self {
    let mut parts: Vec<String> = Vec::new();
    parts.push(persona.identity().to_string());

    let mut guidelines: Vec<&str> = Vec::new();
    for instruction in persona.instructions() {
      guidelines.push(instruction);
    }
    guidelines.push(TOOLS);
    if config.permissions.read_file && config.permissions.edit_file {
      guidelines.push(EDIT);
    }
    if config.general.restrict_write_tools_to_cwd {
      guidelines.push(CWD_RESTRICTED);
    }
    guidelines.push(VERIFY);
    guidelines.push(CONCISE);
    guidelines.push(PATHS);
    guidelines.push(MENTIONS);

    let guidelines_list = guidelines
      .iter()
      .map(|g| format!("- {g}"))
      .collect::<Vec<String>>()
      .join("\n");
    parts.push(format!("Guidelines:\n{guidelines_list}"));

    let cwd = std::env::current_dir()
      .map(|d| d.display().to_string())
      .unwrap_or_else(|_| "unknown".to_string());
    let date_time = get_current_datetime();
    parts.push(format!(
      "Environment:\nCurrent working directory: {cwd}\n{date_time}"
    ));

    let system_prompt = parts.join("\n\n");
    return Self { system_prompt };
  }

  pub fn get_system_prompt(&self) -> &str {
    return &self.system_prompt;
  }
}
