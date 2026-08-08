use thiserror::Error;

#[derive(Error, Debug)]
pub enum CapabilityError {
  #[error("Failed to take screenshot: {reason}.")]
  Screenshot { reason: String },
}

pub type CapabilityErrorResult<T> = Result<T, CapabilityError>;
