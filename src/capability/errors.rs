use thiserror::Error;

#[derive(Error, Debug)]
pub enum CapabilityError {
  #[error("Failed to take screenshot: {reason}.")]
  Screenshot { reason: String },

  #[error("Failed to control the keyboard: {reason}.")]
  Keyboard { reason: String },

  #[error("Failed to query or control a window: {reason}.")]
  Window { reason: String },

  #[error("Failed to control the browser: {reason}.")]
  Browser { reason: String },
}

pub type CapabilityErrorResult<T> = Result<T, CapabilityError>;
