pub mod commands;

use anyhow::{Result, bail};

pub use commands::SlashCommand;

pub fn parse_command(input: &str) -> Result<Option<SlashCommand>> {
  if !input.starts_with('/') {
    return Ok(None);
  }
  let trimmed = input.trim();
  for cmd in SlashCommand::all() {
    if cmd.name() == trimmed {
      return Ok(Some(cmd.clone()));
    }
  }
  bail!("Unknown command '{trimmed}'. Type '/help' for available commands.");
}
