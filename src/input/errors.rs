use thiserror::Error;

#[derive(Error, Debug)]
pub enum InputError {
  #[error("Input is empty")]
  EmptyInput,

  #[error("Cannot read input from stdin: '{0}'.")]
  StdinRead(String),
}

pub type InputResult<T> = Result<T, InputError>;
