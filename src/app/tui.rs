use crate::app::App;
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::prompt::Prompt;
use crate::slash_commands::{SlashCommand, parse_command};
use crate::tui::TUI;
use crate::vlog;

impl App {
  pub async fn run_tui(&mut self) -> RuntimeResult<String> {
    let prompt = Prompt::new();
    vlog!("Prompt: {}", prompt.get_system_prompt());

    let mut terminal = ratatui::init();
    let mut tui = TUI::new();

    while tui.is_running() && !self.shutdown.is_cancelled() {
      terminal
        .draw(|frame| tui.draw(frame))
        .map_err(|e| RuntimeError::TUI(e.to_string()))?;

      let input = tui.read_input().await;
      if let Some(input) = input {
        match parse_command(&input) {
          Ok(Some(SlashCommand::Exit)) => {
            tui.quit();
            break;
          }
          Ok(Some(SlashCommand::Help)) => {
            tui.open_help_popup();
            continue;
          }
          Ok(Some(cmd)) => {
            tui.add_message(format!("Unknown command: {}", cmd.name()));
          }
          Ok(None) => {}
          Err(e) => {
            tui.add_message(format!("Error: {}", e.to_string()));
            continue;
          }
        }

        self.ensure_session_with_system_message().await?;

        self
          .session_manager
          .add_user_message_to_current_session(&input)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;

        match self.process_chat().await {
          Ok(content) => {
            tui.add_message(format!("You: {}", input));
            if !content.is_empty() {
              tui.add_message(format!("Fenrir: {}", content));
            }
          }
          Err(e) => {
            tui.add_message(format!("Error: {}", e.to_string()));
            continue;
          }
        }
      }
    }

    ratatui::restore();
    return Ok(String::new());
  }
}
