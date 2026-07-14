use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuntimeError {
  #[error("Unknown input: {0}")]
  Input(String),
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;
