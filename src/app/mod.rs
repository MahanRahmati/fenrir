pub mod errors;

use std::collections::HashMap;

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

      self.ensure_session_with_system_message(&prompt).await?;

      self
        .session_manager
        .add_user_message_to_current_session(&input)
        .await
        .map_err(|e| RuntimeError::Session(e.to_string()))?;

      print!("{}", self.process_chat_inner().await?);
      println!();
    }

    return Ok(String::new());
  }

  async fn process_chat_inner(&mut self) -> RuntimeResult<String> {
    let mut assistant_content = String::new();

    loop {
      let session = self
        .session_manager
        .get_current_session()
        .ok_or_else(|| RuntimeError::Session("No active session".into()))?;

      let messages = session.messages.clone();
      let tools = self.tool_manager.get_tools_specs();

      let response = self
        .provider_manager
        .chat(&messages, &tools)
        .await
        .map_err(|e| RuntimeError::Provider(e.to_string()))?;

      if let Some(ref content) = response.content {
        assistant_content.push_str(content);
      }

      if response.tool_calls.is_empty() {
        if let Some(ref content) = response.content {
          self
            .session_manager
            .add_assistant_message_to_current_session(content)
            .await
            .map_err(|e| RuntimeError::Session(e.to_string()))?;
        }
        break;
      }

      for tool_call in &response.tool_calls {
        let params: HashMap<String, String> =
          serde_json::from_str(&tool_call.arguments).unwrap_or_default();

        let result = self.tool_manager.execute_tool(&tool_call.name, params);

        let result_str = match result {
          Ok(r) => r,
          Err(e) => format!("Tool error: {}", e),
        };

        self
          .session_manager
          .add_tool_result_to_current_session(&tool_call.name, &result_str)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }
    }

    return Ok(assistant_content);
  }

  async fn ensure_session_with_system_message(
    &mut self,
    prompt: &Prompt,
  ) -> RuntimeResult<()> {
    if self.session_manager.get_current_session().is_some() {
      return Ok(());
    }

    let session_id = self.session_manager.create_session();
    vlog!("Created session with ID: {}", session_id);
    self
      .session_manager
      .add_system_message_to_current_session(prompt.get_system_prompt())
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;

    return Ok(());
  }
}
