mod cli;
pub mod errors;
mod tui;

use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::config::Config;
use crate::database::DB;
use crate::prompt::Prompt;
use crate::provider_manager::ProviderManager;
use crate::session_manager::SessionManager;
use crate::tool_manager::ToolManager;
use crate::tool_manager::tool::parse_tool_params;
use crate::vlog;

pub struct App {
  session_manager: SessionManager,
  provider_manager: ProviderManager,
  tool_manager: ToolManager,
}

impl App {
  pub fn new(config: Config, db: DB) -> Self {
    let session_manager = SessionManager::new(db);
    let tool_manager = ToolManager::new(config.clone());
    let provider_manager = ProviderManager::new(config.clone());
    return App {
      session_manager,
      provider_manager,
      tool_manager,
    };
  }

  async fn ensure_session_with_system_message(&mut self) -> RuntimeResult<()> {
    if self.session_manager.get_current_session().is_some() {
      return Ok(());
    }

    let session_id = self.session_manager.create_session();
    vlog!("Created session with ID: {}", session_id);

    let prompt = Prompt::new();
    vlog!("Prompt: {}", prompt.get_system_prompt());
    self
      .session_manager
      .add_system_message_to_current_session(prompt.get_system_prompt())
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;

    return Ok(());
  }

  async fn process_chat(&mut self) -> RuntimeResult<String> {
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
        vlog!(
          "Tool call: {} with arguments: {}",
          tool_call.name,
          tool_call.arguments
        );

        self
          .session_manager
          .add_tool_call_to_current_session(
            &tool_call.id,
            &tool_call.name,
            &tool_call.arguments,
          )
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;

        let params = parse_tool_params(&tool_call.arguments);
        let result = self.tool_manager.execute_tool(&tool_call.name, params);

        let result_str = match result {
          Ok(r) => {
            vlog!("Tool '{}' result: {}", tool_call.name, r);
            r
          }
          Err(e) => {
            vlog!("Tool '{}' error: {}", tool_call.name, e);
            format!("Tool error: {}", e)
          }
        };

        self
          .session_manager
          .add_tool_result_to_current_session(&tool_call.id, &result_str)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }
    }

    return Ok(assistant_content);
  }
}
