use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
  #[error("Cannot read configuration file: '{0}'.")]
  FileRead(String),

  #[error("Configuration file is invalid: '{0}'.")]
  Parse(String),

  #[error("Cannot save configuration file: '{0}'.")]
  FileWrite(String),
}

pub type ConfigResult<T> = Result<T, ConfigError>;
