use crate::engine::protocol::CommandInfo;
use crate::slash_commands::SlashCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
  New,
  Sessions,
  Personas,
  Models,
  Compact,
  Review,
  Help,
  Exit,
}

impl Command {
  pub fn catalog() -> Vec<CommandInfo> {
    return SlashCommand::all()
      .iter()
      .map(|cmd| match cmd {
        SlashCommand::New => CommandInfo {
          name: "/new".to_string(),
          description: "Create a new session".to_string(),
        },
        SlashCommand::Sessions => CommandInfo {
          name: "/sessions".to_string(),
          description: "List all sessions".to_string(),
        },
        SlashCommand::Personas => CommandInfo {
          name: "/personas".to_string(),
          description: "List all personas".to_string(),
        },
        SlashCommand::Models => CommandInfo {
          name: "/models".to_string(),
          description: "List all available models".to_string(),
        },
        SlashCommand::Compact => CommandInfo {
          name: "/compact".to_string(),
          description: "Compact the current session".to_string(),
        },
        SlashCommand::Review => CommandInfo {
          name: "/review".to_string(),
          description: "Review uncommitted or session changes".to_string(),
        },
        SlashCommand::Help => CommandInfo {
          name: "/help".to_string(),
          description: "Show this help message".to_string(),
        },
        SlashCommand::Q | SlashCommand::Quit | SlashCommand::Exit => {
          CommandInfo {
            name: "/exit".to_string(),
            description: "Exit the application".to_string(),
          }
        }
      })
      .collect();
  }
}

pub fn parse(input: &str) -> Result<Option<Command>, String> {
  if !input.starts_with('/') {
    return Ok(None);
  }
  let trimmed = input.trim();
  let slash = SlashCommand::all()
    .iter()
    .find(|cmd| cmd.name() == trimmed)
    .ok_or_else(|| {
      format!(
        "Unknown command '{trimmed}'. Type '/help' for available commands."
      )
    })?;
  let cmd = match slash {
    SlashCommand::New => Command::New,
    SlashCommand::Sessions => Command::Sessions,
    SlashCommand::Personas => Command::Personas,
    SlashCommand::Models => Command::Models,
    SlashCommand::Compact => Command::Compact,
    SlashCommand::Review => Command::Review,
    SlashCommand::Help => Command::Help,
    SlashCommand::Q | SlashCommand::Quit | SlashCommand::Exit => Command::Exit,
  };
  return Ok(Some(cmd));
}
