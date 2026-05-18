mod errors;
mod prompt;

use crate::app::errors::RuntimeResult;
use crate::app::prompt::Prompt;
use crate::config::Config;

pub struct App {
  config: Config,
}

impl App {
  pub fn new(config: Config) -> Self {
    return App { config };
  }

  pub async fn run(&self) -> RuntimeResult<String> {
    let prompt = Prompt::new();
    return Ok(String::from(prompt.get_system_prompt()));
  }
}
