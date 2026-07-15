use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
  #[error("Database initialization error: {0}")]
  Initialization(String),

  #[error("Database connection error: {0}")]
  Connection(String),

  #[error("Database query error: {0}")]
  Query(String),

  #[error("Migration error: {0}")]
  Migration(String),
}

pub type DatabaseResult<T> = Result<T, DatabaseError>;
