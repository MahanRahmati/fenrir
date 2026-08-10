use thiserror::Error;

#[derive(Error, Debug)]
pub enum CapabilityError {
  #[error("Failed to take screenshot: {reason}.")]
  Screenshot { reason: String },

  #[error("Failed to control the keyboard: {reason}.")]
  Keyboard { reason: String },
}

pub type CapabilityErrorResult<T> = Result<T, CapabilityError>;
