mod errors;

use crate::app::errors::RuntimeResult;
use crate::config::Config;

pub struct App {
  config: Config,
}

impl App {
  pub fn new(config: Config) -> Self {
    return App { config };
  }

  pub async fn run(&self) -> RuntimeResult<String> {
    return Ok(String::from("Hello, World!"));
  }
}
