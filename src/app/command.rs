use crate::persona_manager::Persona;
use crate::session_manager::session::Session;
use crate::slash_commands::SlashCommand;
use crate::slash_commands::SlashCommandResult;
use crate::slash_commands::parse_command;

pub enum AppCommand {
  Exit,
  Help,
  ListSessions,
  LoadSession { id: i64 },
  ListPersonas,
  SelectPersona { name: String },
  New,
}

pub enum AppCommandResult {
  Exit,
  Help(Vec<SlashCommand>),
  Sessions {
    sessions: Vec<Session>,
    current: Option<i64>,
  },
  Session(Session),
  Personas {
    personas: Vec<Persona>,
    current: String,
  },
  Persona(String),
  Noop,
}

impl AppCommand {
  pub fn parse(input: &str) -> SlashCommandResult<Option<Self>> {
    return parse_command(input).map(|command| {
      command.map(|command| match command {
        SlashCommand::Exit => Self::Exit,
        SlashCommand::Help => Self::Help,
        SlashCommand::Sessions => Self::ListSessions,
        SlashCommand::Personas => Self::ListPersonas,
        SlashCommand::New => Self::New,
      })
    });
  }
}
