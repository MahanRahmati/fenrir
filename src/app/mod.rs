pub mod errors;

use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::config::Config;
use crate::database::DB;
use crate::input::errors::InputError;
use crate::input::read_from_stdin;
use crate::prompt::Prompt;
use crate::provider_manager::ProviderManager;
use crate::session_manager::SessionManager;
use crate::slash_commands::{SlashCommand, parse_command};
use crate::tool_manager::ToolManager;
use crate::vlog;

pub struct App {
  session_manager: SessionManager,
  provider_manager: ProviderManager,
  tool_manager: ToolManager,
}

impl App {
  pub fn new(config: Config, db: DB) -> Self {
    let session_manager = SessionManager::new(db);
    let tool_manager = ToolManager::new();
    let provider_manager = ProviderManager::new(config);
    return App {
      session_manager,
      provider_manager,
      tool_manager,
    };
  }

  pub async fn run(&mut self) -> RuntimeResult<String> {
    let prompt = Prompt::new();
    vlog!("Prompt: {}", prompt.get_system_prompt());

    loop {
      let input = match read_from_stdin() {
        Ok(input) => input,
        Err(InputError::EmptyInput) => continue,
        Err(e) => return Err(RuntimeError::Input(e.to_string())),
      };

      match parse_command(&input) {
        Ok(Some(SlashCommand::Exit)) => break,
        Ok(Some(SlashCommand::Help)) => {
          println!("Available commands:");
          for cmd in SlashCommand::all() {
            println!("  {}: {}", cmd.name(), cmd.description());
          }
          continue;
        }
        Ok(Some(cmd)) => println!("Unknown command: {}", cmd.name()),
        Ok(None) => {}
        Err(e) => {
          println!("Error: {}", e);
          continue;
        }
      }

      if self.session_manager.get_current_session().is_none() {
        let session_id = self.session_manager.create_session();
        vlog!("Created session with ID: {}", session_id);
        match self
          .session_manager
          .add_system_message_to_current_session(prompt.get_system_prompt())
          .await
        {
          Ok(_) => {}
          Err(e) => return Err(RuntimeError::Session(e.to_string())),
        }
      }

      match self
        .session_manager
        .add_user_message_to_current_session(&input)
        .await
      {
        Ok(_) => {}
        Err(e) => return Err(RuntimeError::Session(e.to_string())),
      }

      self.process_chat().await;
    }

    return Ok(String::new());
  }

  async fn process_chat(&mut self) {
    if let Some(session) = self.session_manager.get_current_session() {
      for message in session.messages.iter() {
        println!("Message: {}", message.content);
      }
    }
  }
}
