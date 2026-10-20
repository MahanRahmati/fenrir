use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuntimeError {
  #[error("Unknown input: {0}")]
  Input(String),

  #[error("Session error: {0}")]
  Session(String),

  #[error("Persona error: {0}")]
  Persona(String),

  #[error("Provider error: {0}")]
  Provider(String),

  #[error("TUI error: {0}")]
  Tui(String),

  #[error("Compaction error: {0}")]
  Compaction(String),

  #[error("Subagent error: {0}")]
  Subagent(String),
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;
