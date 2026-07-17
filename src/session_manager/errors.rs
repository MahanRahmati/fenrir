use thiserror::Error;

#[derive(Error, Debug)]
pub enum SessionManagerError {
  #[error("No current session active")]
  NoCurrentSession,

  #[error("Failed to load session from database: {0}")]
  SessionLoadFailed(String),

  #[error("Database error: {0}")]
  DatabaseError(String),
}

pub type SessionManagerResult<T> = Result<T, SessionManagerError>;
