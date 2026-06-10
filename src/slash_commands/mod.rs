pub mod commands;
pub mod errors;

pub use commands::SlashCommand;
pub use errors::{SlashCommandError, SlashCommandResult};

pub fn parse_command(input: &str) -> SlashCommandResult<Option<SlashCommand>> {
  if !input.starts_with('/') {
    return Ok(None);
  }
  let trimmed = input.trim();
  for cmd in SlashCommand::all() {
    if cmd.name() == trimmed {
      return Ok(Some(cmd.clone()));
    }
  }
  return Err(SlashCommandError::UnknownCommand(trimmed.to_string()));
}
