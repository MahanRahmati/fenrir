use std::io::stdout;

use ratatui::crossterm;
use ratatui::crossterm::event::{
  self, DisableMouseCapture, EnableMouseCapture, Event,
  KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
  PushKeyboardEnhancementFlags,
};

use crate::app::command::{AppCommand, AppCommandResult};
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::app::{App, AppEvent};
use crate::tui::Tui;

impl App {
  pub async fn run_tui(&mut self) -> RuntimeResult<String> {
    let mut terminal = ratatui::init();
    crossterm::execute!(stdout(), EnableMouseCapture)
      .map_err(|e| RuntimeError::Tui(e.to_string()))?;
    crossterm::execute!(
      stdout(),
      PushKeyboardEnhancementFlags(
        KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
      )
    )
    .map_err(|e| RuntimeError::Tui(e.to_string()))?;
    let mut tui = Tui::new();
    tui.set_persona(self.persona_manager.current().name());

    while tui.is_running() && !self.shutdown.is_cancelled() {
      terminal
        .draw(|frame| tui.draw(frame))
        .map_err(|e| RuntimeError::Tui(e.to_string()))?;

      let input = tui.read_input().await;
      if let Some(session_id) = tui.get_selected_session() {
        if let AppCommandResult::Session(session) = self
          .handle_command(AppCommand::LoadSession { id: session_id })
          .await?
        {
          tui.update_messages(Some(&session));
        }
        continue;
      }
      if let Some(name) = tui.get_selected_persona() {
        self
          .handle_command(AppCommand::SelectPersona { name: name.clone() })
          .await?;
        tui.set_persona(&name);
        continue;
      }
      if let Some(input) = input {
        let command = match AppCommand::parse(&input) {
          Ok(Some(command)) => Some(command),
          Ok(None) => None,
          Err(e) => {
            tui.add_message(format!("Error: {e}"));
            continue;
          }
        };

        if let Some(command) = command {
          match self.handle_command(command).await? {
            AppCommandResult::Exit => {
              tui.quit();
              break;
            }
            AppCommandResult::Help(commands) => {
              tui.open_help_popup(commands);
            }
            AppCommandResult::Sessions { sessions, current } => {
              tui.open_sessions_popup(sessions, current);
            }
            AppCommandResult::Personas { personas, current } => {
              let current_persona =
                personas.iter().find(|p| p.name() == current);
              tui.open_personas_popup(&personas, current_persona);
            }
            AppCommandResult::Persona(name) => {
              tui.set_persona(&name);
              tui.update_messages(None);
            }
            _ => {}
          }
          continue;
        }

        self.ensure_session_with_system_message().await?;

        self
          .session_manager
          .add_user_message_to_current_session(&input)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;

        tui.add_message(input);

        tui.set_processing(true);
        terminal
          .draw(|frame| tui.draw(frame))
          .map_err(|e| RuntimeError::Tui(e.to_string()))?;

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
            tui.set_processing(false);
            tui.set_usage(self.last_usage());
            terminal.draw(|frame| tui.draw(frame)).ok();
          }
          Err(e) => {
            tui.set_processing(false);
            tui.add_message(format!("Error: {e}"));
            continue;
          }
        }
      }
    }

    crossterm::execute!(stdout(), DisableMouseCapture)
      .map_err(|e| RuntimeError::Tui(e.to_string()))?;
    crossterm::execute!(stdout(), PopKeyboardEnhancementFlags)
      .map_err(|e| RuntimeError::Tui(e.to_string()))?;
    ratatui::restore();
    return Ok(String::new());
  }
}
