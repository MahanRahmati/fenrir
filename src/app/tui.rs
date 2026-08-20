use std::io::stdout;

use ratatui::crossterm;
use ratatui::crossterm::event::{
  self, DisableMouseCapture, EnableMouseCapture, Event,
  KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
  PushKeyboardEnhancementFlags,
};
use tokio_util::sync::CancellationToken;

use crate::app::command::{AppCommand, AppCommandResult};
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::app::{App, AppEvent, EventSink};
use crate::tui::Tui;

struct TuiSink {
  tui: Tui,
  terminal: ratatui::DefaultTerminal,
}

#[async_trait::async_trait]
impl EventSink for TuiSink {
  fn on_event(&mut self, event: AppEvent) {
    match event {
      AppEvent::Token { text, reasoning } => {
        if reasoning {
          self.tui.ensure_reasoning_line();
        } else {
          self.tui.ensure_content_line();
        }
        self.tui.append_to_last_message(&text);
        self.terminal.draw(|frame| self.tui.draw(frame)).ok();
      }
      AppEvent::ToolCall { name, arguments } => {
        self.tui.add_tool_call(name, arguments);
        self.terminal.draw(|frame| self.tui.draw(frame)).ok();
      }
      AppEvent::ToolResult { result, error } => {
        self.tui.add_tool_result(result, error);
        self.terminal.draw(|frame| self.tui.draw(frame)).ok();
      }
    }
  }

  async fn confirm(&mut self, message: &str) -> bool {
    self.tui.open_confirm_popup(message.to_string());
    self.terminal.draw(|frame| self.tui.draw(frame)).ok();
    while self.tui.confirm_popup_open() {
      if let Ok(Event::Key(key)) = event::read() {
        self.tui.handle_confirm_event(key.code);
      }
      self.terminal.draw(|frame| self.tui.draw(frame)).ok();
    }
    return self.tui.get_confirmation();
  }
}

impl App {
  pub async fn run_tui(&mut self) -> RuntimeResult<String> {
    let mut sink = TuiSink {
      tui: Tui::new(),
      terminal: ratatui::init(),
    };
    crossterm::execute!(stdout(), EnableMouseCapture)
      .map_err(|e| RuntimeError::Tui(e.to_string()))?;
    crossterm::execute!(
      stdout(),
      PushKeyboardEnhancementFlags(
        KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
      )
    )
    .map_err(|e| RuntimeError::Tui(e.to_string()))?;
    sink.tui.set_persona(self.persona_manager.current().name());

    while sink.tui.is_running() && !self.shutdown.is_cancelled() {
      sink
        .terminal
        .draw(|frame| sink.tui.draw(frame))
        .map_err(|e| RuntimeError::Tui(e.to_string()))?;

      let input = sink.tui.read_input().await;
      if let Some(session_id) = sink.tui.get_selected_session() {
        if let AppCommandResult::Session(session) = self
          .handle_command(AppCommand::LoadSession { id: session_id })
          .await?
        {
          sink.tui.update_messages(Some(&session));
        }
        continue;
      }
      if let Some(name) = sink.tui.get_selected_persona() {
        self
          .handle_command(AppCommand::SelectPersona { name: name.clone() })
          .await?;
        sink.tui.set_persona(&name);
        continue;
      }
      if let Some(input) = input {
        let command = match AppCommand::parse(&input) {
          Ok(Some(command)) => Some(command),
          Ok(None) => None,
          Err(e) => {
            sink.tui.add_message(format!("Error: {e}"));
            continue;
          }
        };

        if let Some(command) = command {
          match self.handle_command(command).await? {
            AppCommandResult::Exit => {
              sink.tui.quit();
              break;
            }
            AppCommandResult::Help(commands) => {
              sink.tui.open_help_popup(commands);
            }
            AppCommandResult::Sessions { sessions, current } => {
              sink.tui.open_sessions_popup(sessions, current);
            }
            AppCommandResult::Personas { personas, current } => {
              let current_persona =
                personas.iter().find(|p| p.name() == current);
              sink.tui.open_personas_popup(&personas, current_persona);
            }
            AppCommandResult::Persona(name) => {
              sink.tui.set_persona(&name);
              sink.tui.update_messages(None);
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

        sink.tui.add_message(input);

        sink.tui.set_processing(true);
        sink
          .terminal
          .draw(|frame| sink.tui.draw(frame))
          .map_err(|e| RuntimeError::Tui(e.to_string()))?;

        let cancel = CancellationToken::new();
        match self.process_chat(&mut sink, &cancel).await {
          Ok(_) => {
            sink.tui.set_processing(false);
            sink.tui.set_usage(self.last_usage());
            sink.terminal.draw(|frame| sink.tui.draw(frame)).ok();
          }
          Err(e) => {
            sink.tui.set_processing(false);
            sink.tui.add_message(format!("Error: {e}"));
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
