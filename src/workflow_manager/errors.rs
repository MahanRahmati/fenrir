use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowManagerError {
  #[error("Failed to execute workflow '{name}': {reason}.")]
  ExecutionFailed { name: String, reason: String },
}

pub type WorkflowManagerResult<T> = Result<T, WorkflowManagerError>;
