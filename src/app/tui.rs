use std::io::stdout;

use ratatui::crossterm;
use ratatui::crossterm::event::{
  self, DisableMouseCapture, EnableMouseCapture, Event,
};

use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::app::{App, AppEvent};
use crate::slash_commands::{SlashCommand, parse_command};
use crate::tui::Tui;

impl App {
  pub async fn run_tui(&mut self) -> RuntimeResult<String> {
    let mut terminal = ratatui::init();
    crossterm::execute!(stdout(), EnableMouseCapture)
      .map_err(|e| RuntimeError::Tui(e.to_string()))?;
    let mut tui = Tui::new();

    while tui.is_running() && !self.shutdown.is_cancelled() {
      terminal
        .draw(|frame| tui.draw(frame))
        .map_err(|e| RuntimeError::Tui(e.to_string()))?;

      let input = tui.read_input();
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
      if let Some(persona_index) = tui.get_selected_persona() {
        let personas = self.persona_manager.list();
        let persona = &personas[persona_index];
        self
          .persona_manager
          .set(persona.name())
          .map_err(RuntimeError::Persona)?;
        tui.add_message(format!("Persona set to: {}", persona.name()));
        continue;
      }
      if let Some(input) = input {
        match parse_command(&input) {
          Ok(Some(SlashCommand::Exit)) => {
            tui.quit();
            break;
          }
          Ok(Some(SlashCommand::Help)) => {
            let commands = self.available_commands();
            tui.open_help_popup(commands);
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
          Ok(Some(SlashCommand::Personas)) => {
            if self.session_manager.get_current_session().is_some() {
              continue;
            }
            tui.open_personas_popup(
              &self.persona_manager.list(),
              Some(self.persona_manager.current()),
            );
            continue;
          }
          Ok(Some(SlashCommand::New)) => {
            self.session_manager.clear_current_session();
            self.persona_manager.reset();
            tui.update_messages(None);
            continue;
          }
          Ok(None) => {}
          Err(e) => {
            tui.add_message(format!("Error: {e}"));
            continue;
          }
        }

        self.ensure_session_with_system_message().await?;

        self
          .session_manager
          .add_user_message_to_current_session(&input)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;

        tui.add_message(input);

        let handler = |event: AppEvent| -> bool {
          match event {
            AppEvent::Token { text, reasoning } => {
              if reasoning {
                tui.ensure_reasoning_line();
              } else {
                tui.ensure_content_line();
              }
              tui.append_to_last_message(&text);
              terminal.draw(|frame| tui.draw(frame)).ok();
              return true;
            }
            AppEvent::Confirm(msg) => {
              tui.open_confirm_popup(msg);
              terminal.draw(|frame| tui.draw(frame)).ok();
              while tui.confirm_popup_open() {
                if let Ok(Event::Key(key)) = event::read() {
                  tui.handle_confirm_event(key.code);
                }
                terminal.draw(|frame| tui.draw(frame)).ok();
              }
              return tui.get_confirmation();
            }
            AppEvent::ToolCall { name, arguments } => {
              tui.add_tool_call(name, arguments);
              terminal.draw(|frame| tui.draw(frame)).ok();
              return true;
            }
            AppEvent::ToolResult { result, error } => {
              tui.add_tool_result(result, error);
              terminal.draw(|frame| tui.draw(frame)).ok();
              return true;
            }
          }
        };

        match self.process_chat(handler).await {
          Ok(_) => {
            terminal.draw(|frame| tui.draw(frame)).ok();
          }
          Err(e) => {
            tui.add_message(format!("Error: {e}"));
            continue;
          }
        }
      }
    }

    crossterm::execute!(stdout(), DisableMouseCapture)
      .map_err(|e| RuntimeError::Tui(e.to_string()))?;
    ratatui::restore();
    return Ok(String::new());
  }
}
