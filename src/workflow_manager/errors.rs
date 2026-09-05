use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowManagerError {
  #[error("Failed to execute workflow '{name}': {reason}.")]
  ExecutionFailed { name: String, reason: String },

  #[error("Could not find workflow '{name}'.")]
  NotFound { name: String },
}

pub type WorkflowManagerResult<T> = Result<T, WorkflowManagerError>;
