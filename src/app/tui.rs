use crate::app::App;
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::prompt::Prompt;
use crate::slash_commands::{SlashCommand, parse_command};
use crate::tui::Tui;
use crate::vlog;

impl App {
  pub async fn run_tui(&mut self) -> RuntimeResult<String> {
    let prompt = Prompt::new();
    vlog!("Prompt: {}", prompt.get_system_prompt());

    let mut terminal = ratatui::init();
    let mut tui = Tui::new();

    while tui.is_running() && !self.shutdown.is_cancelled() {
      terminal
        .draw(|frame| tui.draw(frame))
        .map_err(|e| RuntimeError::Tui(e.to_string()))?;

      let input = tui.read_input().await;
      if let Some(session_id) = tui.get_selected_session() {
        self
          .session_manager
          .load_session_from_db(session_id)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
        let current_session = self.session_manager.get_current_session();
        tui.update_messages(current_session);
        continue;
      }
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
          Ok(Some(SlashCommand::Sessions)) => {
            let sessions = self
              .session_manager
              .list_sessions()
              .await
              .map_err(|e| RuntimeError::Session(e.to_string()))?;
            let current_session = self.session_manager.get_current_session();
            tui.open_sessions_popup(sessions, current_session);
            continue;
          }
          Ok(Some(cmd)) => {
            tui.add_message(format!("Unknown command: {}", cmd.name()));
            continue;
          }
          Ok(None) => {}
          Err(e) => {
            tui.add_message(format!("Error: {}", e));
            continue;
          }
        }

        self.ensure_session_with_system_message().await?;

        self
          .session_manager
          .add_user_message_to_current_session(&input)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;

        tui.add_message(format!("You: {}", input));
        tui.add_message("Fenrir: ".to_string());

        match self
          .process_chat(|token| {
            tui.append_to_last_message(token);
            terminal.draw(|frame| tui.draw(frame)).ok();
          })
          .await
        {
          Ok(_) => {
            terminal.draw(|frame| tui.draw(frame)).ok();
          }
          Err(e) => {
            tui.add_message(format!("Error: {}", e));
            continue;
          }
        }
      }
    }

    ratatui::restore();
    return Ok(String::new());
  }
}
