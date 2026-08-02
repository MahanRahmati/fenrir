use crate::config::Config;
use crate::tool_manager::tool::ToolSpec;
use crate::utils::datetime::get_current_datetime;

const IDENTITY: &str = "You are Fenrir, a helpful AI assistant.";
const TOOLS: &str = "Use tools when appropriate.";
const EDIT: &str = "Before editing files, read them with `append_loc=true` for accurate line numbers. Prefer many small edits over one large one.";
const VERIFY: &str = "After making changes, use available tools to verify they work (e.g. `rust` tool for Rust projects if available).";
const CWD_RESTRICTED: &str = "Tools are restricted to the current working directory. Always use paths within the current directory; paths outside the current working directory are blocked.";
const CONCISE: &str = "Be concise and direct.";
const PATHS: &str = "Show file paths clearly when working with files.";

pub struct Prompt {
  system_prompt: String,
}

impl Prompt {
  pub fn new(config: Config, tool_specs: &[ToolSpec]) -> Self {
    let mut parts: Vec<String> = Vec::new();
    parts.push(IDENTITY.to_string());

    if !tool_specs.is_empty() {
      let tools_list = tool_specs
        .iter()
        .map(|t| format!("- {}: {}", t.name, t.description))
        .collect::<Vec<String>>()
        .join("\n");
      parts.push(format!("Available tools:\n{}", tools_list));
    }

    let mut guidelines: Vec<&str> = Vec::new();
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

    let guidelines_list = guidelines
      .iter()
      .map(|g| format!("- {}", g))
      .collect::<Vec<String>>()
      .join("\n");
    parts.push(format!("Guidelines:\n{}", guidelines_list));

    let cwd = std::env::current_dir()
      .map(|d| d.display().to_string())
      .unwrap_or_else(|_| "unknown".to_string());
    let date_time = get_current_datetime();
    parts.push(format!(
      "Environment:\nCurrent working directory: {}\n{}",
      cwd, date_time
    ));

    let system_prompt = parts.join("\n\n");
    return Self { system_prompt };
  }

  pub fn get_system_prompt(&self) -> &str {
    return &self.system_prompt;
  }
}
