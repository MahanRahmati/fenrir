use std::time::Duration;

use anyhow::Error;
use reqwest::StatusCode;

use crate::config::NetworkConfig;
use crate::network::NetworkError;

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
  pub max_retries: u32,
  pub base_delay: Duration,
  pub max_delay: Duration,
}

#[derive(Debug, Clone, Copy)]
pub struct RetryAdvice {
  pub retry_after: Option<Duration>,
}

#[derive(Debug, Clone, Copy)]
pub enum RetryDelay {
  Wait(Duration),
  TooLong(Duration),
}

impl RetryPolicy {
  pub fn from_config(config: &NetworkConfig) -> Self {
    return Self {
      max_retries: config.max_retries,
      base_delay: Duration::from_secs(config.retry_base_delay_seconds),
      max_delay: Duration::from_secs(config.retry_max_delay_seconds),
    };
  }

  pub fn max_attempts(&self) -> u32 {
    return self.max_retries.saturating_add(1);
  }

  pub fn delay(&self, retry: u32, advice: RetryAdvice) -> RetryDelay {
    if let Some(retry_after) = advice.retry_after {
      if retry_after > self.max_delay {
        return RetryDelay::TooLong(retry_after);
      }
      return RetryDelay::Wait(retry_after);
    }

    let exponent = retry.saturating_sub(1).min(16);
    let seconds = self.base_delay.as_secs().saturating_mul(1u64 << exponent);
    let capped = seconds.min(self.max_delay.as_secs());
    return RetryDelay::Wait(Duration::from_secs(capped));
  }
}

pub fn classify(error: &Error) -> Option<RetryAdvice> {
  for cause in error.chain() {
    let Some(network) = cause.downcast_ref::<NetworkError>() else {
      continue;
    };
    return match network {
      NetworkError::Transport(source)
        if source.is_connect()
          || source.is_timeout()
          || source.is_request() =>
      {
        Some(RetryAdvice { retry_after: None })
      }
      NetworkError::Timeout { .. } => Some(RetryAdvice { retry_after: None }),
      NetworkError::Status {
        status,
        retry_after,
        ..
      } if is_retryable_status(*status) => Some(RetryAdvice {
        retry_after: *retry_after,
      }),
      _ => None,
    };
  }
  return None;
}

pub fn reason(error: &Error) -> String {
  for cause in error.chain() {
    let Some(network) = cause.downcast_ref::<NetworkError>() else {
      continue;
    };
    return match network {
      NetworkError::Transport(source) if source.is_timeout() => {
        "request timed out".to_string()
      }
      NetworkError::Transport(source) if source.is_connect() => {
        "connection failed".to_string()
      }
      NetworkError::Transport(_) => "network error".to_string(),
      NetworkError::Timeout { .. } => "request timed out".to_string(),
      NetworkError::Status { status, .. } => {
        format!("server returned {status}")
      }
    };
  }
  return error.to_string();
}

pub fn is_retryable_status(status: StatusCode) -> bool {
  return matches!(
    status.as_u16(),
    408 | 425 | 429 | 500 | 502 | 503 | 504 | 529
  );
}
