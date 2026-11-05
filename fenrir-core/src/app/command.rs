use anyhow::Result;
use uuid::Uuid;

use crate::slash_commands::SlashCommand;
use crate::slash_commands::parse_command;

pub enum AppCommand {
  Exit,
  Help,
  ListSessions,
  ListPersonas,
  ListModels,
  Compact,
  Review,
  New,
}

pub enum CommandOutcome {
  Continue,
  Exit,
  Activate(Option<Uuid>),
}

impl AppCommand {
  pub fn parse(input: &str) -> Result<Option<Self>> {
    return parse_command(input).map(|command| {
      command.map(|command| match command {
        SlashCommand::Q => Self::Exit,
        SlashCommand::Quit => Self::Exit,
        SlashCommand::Exit => Self::Exit,
        SlashCommand::Help => Self::Help,
        SlashCommand::Sessions => Self::ListSessions,
        SlashCommand::Personas => Self::ListPersonas,
        SlashCommand::Models => Self::ListModels,
        SlashCommand::Compact => Self::Compact,
        SlashCommand::Review => Self::Review,
        SlashCommand::New => Self::New,
      })
    });
  }
}
