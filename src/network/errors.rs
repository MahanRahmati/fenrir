use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkError {
  #[error("Failed to connect to service: '{0}'.")]
  RequestFailed(String),

  #[error("Service returned {status}: {body}.")]
  ResponseError { status: String, body: String },
}

pub type NetworkResult<T> = Result<T, NetworkError>;
