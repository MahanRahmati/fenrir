use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
  #[error(
    "Cannot read configuration file: '{0}'. Please check file permissions and ensure the file exists."
  )]
  FileRead(String),

  #[error(
    "Configuration file is invalid: '{0}'. Please check the syntax and ensure all required fields are present."
  )]
  Parse(String),
}

pub type ConfigResult<T> = Result<T, ConfigError>;
