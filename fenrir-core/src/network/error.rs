use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

use reqwest::StatusCode;

#[derive(Debug)]
pub enum NetworkError {
  Transport(reqwest::Error),
  Timeout {
    seconds: u64,
  },
  Status {
    status: StatusCode,
    body: String,
    retry_after: Option<Duration>,
  },
}

impl Display for NetworkError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    return match self {
      NetworkError::Transport(error) => write!(f, "{error}"),
      NetworkError::Timeout { seconds } => {
        write!(f, "request timed out after {seconds}s")
      }
      NetworkError::Status { status, body, .. } => {
        write!(f, "Service returned {status}: {body}")?;
        if let Some(hint) = status_hint(*status) {
          write!(f, " ({hint})")?;
        }
        return Ok(());
      }
    };
  }
}

impl Error for NetworkError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    return match self {
      NetworkError::Transport(error) => Some(error),
      NetworkError::Timeout { .. } | NetworkError::Status { .. } => None,
    };
  }
}

fn status_hint(status: StatusCode) -> Option<&'static str> {
  return match status.as_u16() {
    401 => Some("check the API key"),
    402 => Some("the account may be out of credits"),
    403 => Some("access denied"),
    429 => Some("rate limited by the provider"),
    500..=599 => Some("the provider is having trouble"),
    _ => None,
  };
}
