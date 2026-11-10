use anyhow::{Result, bail};
use uuid::Uuid;

use crate::engine::protocol::CommandInfo;
use crate::provider_manager::Model;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub enum Effect {
  CreateSession {
    persona: Option<String>,
    model: Option<Model>,
  },
  ListSessions,
  ListPersonas,
  ListModels,
  Compact {
    session_id: Uuid,
  },
  StartReview {
    session_id: Option<Uuid>,
  },
  ShowHelp,
  Exit,
}

struct CommandEntry {
  command: Command,
  name: &'static str,
  description: &'static str,
}

const COMMANDS: &[CommandEntry] = &[
  CommandEntry {
    command: Command::New,
    name: "/new",
    description: "Create a new session",
  },
  CommandEntry {
    command: Command::Sessions,
    name: "/sessions",
    description: "List all sessions",
  },
  CommandEntry {
    command: Command::Personas,
    name: "/personas",
    description: "List all personas",
  },
  CommandEntry {
    command: Command::Models,
    name: "/models",
    description: "List all available models",
  },
  CommandEntry {
    command: Command::Compact,
    name: "/compact",
    description: "Compact the current session",
  },
  CommandEntry {
    command: Command::Review,
    name: "/review",
    description: "Review uncommitted or session changes",
  },
  CommandEntry {
    command: Command::Help,
    name: "/help",
    description: "Show this help message",
  },
  CommandEntry {
    command: Command::Exit,
    name: "/q",
    description: "Exit the application",
  },
  CommandEntry {
    command: Command::Exit,
    name: "/quit",
    description: "Exit the application",
  },
  CommandEntry {
    command: Command::Exit,
    name: "/exit",
    description: "Exit the application",
  },
];

impl Command {
  pub fn parse(input: &str) -> Result<Option<Self>> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
      return Ok(None);
    }
    let command = match trimmed {
      "/new" => Self::New,
      "/sessions" => Self::Sessions,
      "/personas" => Self::Personas,
      "/models" => Self::Models,
      "/compact" => Self::Compact,
      "/review" => Self::Review,
      "/help" => Self::Help,
      "/q" | "/quit" | "/exit" => Self::Exit,
      other => {
        bail!("Unknown command '{other}'. Type '/help' for available commands.")
      }
    };
    return Ok(Some(command));
  }

  pub fn effect(&self, session_id: Option<Uuid>) -> Result<Effect> {
    return match self {
      Self::New => Ok(Effect::CreateSession {
        persona: None,
        model: None,
      }),
      Self::Sessions => Ok(Effect::ListSessions),
      Self::Personas => Ok(Effect::ListPersonas),
      Self::Models => Ok(Effect::ListModels),
      Self::Compact => {
        let Some(session_id) = session_id else {
          bail!("No session selected");
        };
        Ok(Effect::Compact { session_id })
      }
      Self::Review => Ok(Effect::StartReview { session_id }),
      Self::Help => Ok(Effect::ShowHelp),
      Self::Exit => Ok(Effect::Exit),
    };
  }

  pub fn starts_turn(&self) -> bool {
    return matches!(self, Self::Compact | Self::Review);
  }

  pub fn catalog() -> Vec<CommandInfo> {
    return COMMANDS.iter().map(CommandEntry::info).collect();
  }

  pub fn available_commands(has_session: bool) -> Vec<CommandInfo> {
    return COMMANDS
      .iter()
      .filter(|entry| entry.command.is_available(has_session))
      .map(CommandEntry::info)
      .collect();
  }

  fn is_available(&self, has_session: bool) -> bool {
    return !matches!(self, Self::Compact) || has_session;
  }
}

impl CommandEntry {
  fn info(&self) -> CommandInfo {
    return CommandInfo {
      name: self.name.to_string(),
      description: self.description.to_string(),
    };
  }
}
