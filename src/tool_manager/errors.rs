use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolManagerError {
  #[error("Failed to execute tool '{name}': {reason}.")]
  ExecutionFailed { name: String, reason: String },

  #[error("Could not find tool '{name}'.")]
  NotFound { name: String },

  #[error("Missing parameter '{param}' for tool '{name}'.")]
  MissingParameter { param: String, name: String },

  #[error("Tool '{name}' timed out.")]
  Timeout { name: String },
}

pub type ToolManagerResult<T> = Result<T, ToolManagerError>;
