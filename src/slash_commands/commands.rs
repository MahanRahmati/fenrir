#[derive(Debug, Clone)]
pub enum SlashCommand {
  New,
  Sessions,
  Personas,
  Help,
  Exit,
}

impl SlashCommand {
  pub fn name(&self) -> &'static str {
    return match self {
      SlashCommand::New => "/new",
      SlashCommand::Sessions => "/sessions",
      SlashCommand::Personas => "/personas",
      SlashCommand::Help => "/help",
      SlashCommand::Exit => "/exit",
    };
  }

  pub fn description(&self) -> &'static str {
    return match self {
      SlashCommand::New => "Create a new session",
      SlashCommand::Sessions => "List all sessions",
      SlashCommand::Personas => "List all personas",
      SlashCommand::Help => "Show this help message",
      SlashCommand::Exit => "Exit the application",
    };
  }

  pub fn all() -> &'static [SlashCommand] {
    return &[
      SlashCommand::New,
      SlashCommand::Sessions,
      SlashCommand::Personas,
      SlashCommand::Help,
      SlashCommand::Exit,
    ];
  }
}
