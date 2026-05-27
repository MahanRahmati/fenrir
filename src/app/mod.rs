mod errors;

use crate::app::errors::RuntimeResult;
use crate::config::Config;
use crate::prompt::Prompt;
use crate::session_manager::SessionManager;
use crate::vlog;

pub struct App {
  config: Config,
  session_manager: SessionManager,
}

impl App {
  pub fn new(config: Config) -> Self {
    let session_manager = SessionManager::new(config.get_sessions_folder());
    return App {
      config,
      session_manager,
    };
  }

  pub async fn run(&mut self) -> RuntimeResult<String> {
    let prompt = Prompt::new();
    vlog!("Prompt: {}", prompt.get_system_prompt());
    let session_id = self.session_manager.create_session();
    vlog!("Created session with ID: {}", session_id);
    return Ok(String::new());
  }
}
