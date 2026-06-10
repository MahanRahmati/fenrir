#[derive(Debug, Clone)]
pub enum SlashCommand {
  New,
  Sessions,
  Help,
  Exit,
}

impl SlashCommand {
  pub fn name(&self) -> &'static str {
    return match self {
      SlashCommand::New => "/new",
      SlashCommand::Sessions => "/sessions",
      SlashCommand::Help => "/help",
      SlashCommand::Exit => "/exit",
    };
  }

  pub fn description(&self) -> &'static str {
    return match self {
      SlashCommand::New => "Create a new session",
      SlashCommand::Sessions => "List all sessions",
      SlashCommand::Help => "Show this help message",
      SlashCommand::Exit => "Exit the application",
    };
  }

  pub fn all() -> &'static [SlashCommand] {
    return &[
      SlashCommand::New,
      SlashCommand::Sessions,
      SlashCommand::Help,
      SlashCommand::Exit,
    ];
  }
}
