#[derive(Debug, Clone)]
pub enum SlashCommand {
  New,
  Sessions,
  Personas,
  Compact,
  Help,
  Exit,
}

impl SlashCommand {
  pub fn name(&self) -> &'static str {
    return match self {
      SlashCommand::New => "/new",
      SlashCommand::Sessions => "/sessions",
      SlashCommand::Personas => "/personas",
      SlashCommand::Compact => "/compact",
      SlashCommand::Help => "/help",
      SlashCommand::Exit => "/exit",
    };
  }

  pub fn description(&self) -> &'static str {
    return match self {
      SlashCommand::New => "Create a new session",
      SlashCommand::Sessions => "List all sessions",
      SlashCommand::Personas => "List all personas",
      SlashCommand::Compact => "Compact the current session",
      SlashCommand::Help => "Show this help message",
      SlashCommand::Exit => "Exit the application",
    };
  }

  pub fn all() -> &'static [SlashCommand] {
    return &[
      SlashCommand::New,
      SlashCommand::Sessions,
      SlashCommand::Personas,
      SlashCommand::Compact,
      SlashCommand::Help,
      SlashCommand::Exit,
    ];
  }
}
