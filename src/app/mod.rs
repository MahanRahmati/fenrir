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
}
