pub mod errors;

use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::config::Config;
use crate::database::DB;
use crate::input::errors::InputError;
use crate::input::read_from_stdin;
use crate::prompt::Prompt;
use crate::session_manager::SessionManager;
use crate::slash_commands::{SlashCommand, parse_command};
use crate::tool_manager::ToolManager;
use crate::vlog;

pub struct App {
  config: Config,
  session_manager: SessionManager,
  tool_manager: ToolManager,
}

impl App {
  pub fn new(config: Config, db: DB) -> Self {
    let session_manager = SessionManager::new(db);
    let tool_manager = ToolManager::new();
    return App {
      config,
      session_manager,
      tool_manager,
    };
  }

  pub async fn run(&mut self) -> RuntimeResult<String> {
    let prompt = Prompt::new();
    vlog!("Prompt: {}", prompt.get_system_prompt());
    let session_id = self.session_manager.create_session();
    vlog!("Created session with ID: {}", session_id);

    loop {
      let input = match read_from_stdin() {
        Ok(input) => input,
        Err(InputError::EmptyInput) => continue,
        Err(e) => return Err(RuntimeError::Input(e.to_string())),
      };

      self
        .session_manager
        .add_user_message_to_current_session(&input)
        .await;

      match parse_command(&input) {
        Ok(Some(SlashCommand::Exit)) => break,
        Ok(Some(cmd)) => println!("Unknown command: {}", cmd.name()),
        Ok(None) => println!("Assistant: {}", input),
        Err(e) => println!("Error: {}", e),
      }
    }

    return Ok(String::new());
  }
}
