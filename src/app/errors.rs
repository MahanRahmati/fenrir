use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuntimeError {
  #[error("Unknown input: {0}")]
  Input(String),

  #[error("Session error: {0}")]
  Session(String),

  #[error("Provider error: {0}")]
  Provider(String),

  #[error("TUI error: {0}")]
  TUI(String),
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;
