mod errors;
mod prompt;

use crate::app::errors::RuntimeResult;
use crate::app::prompt::Prompt;
use crate::config::Config;
use crate::session_manager::SessionManager;

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

  pub async fn run(&self) -> RuntimeResult<String> {
    let prompt = Prompt::new();
    return Ok(String::from(prompt.get_system_prompt()));
  }
}
