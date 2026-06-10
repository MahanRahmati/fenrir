use thiserror::Error;

#[derive(Error, Debug)]
pub enum SlashCommandError {
  #[error("Unknown command '{0}'. Type '/help' for available commands.")]
  UnknownCommand(String),
}

pub type SlashCommandResult<T> = Result<T, SlashCommandError>;
