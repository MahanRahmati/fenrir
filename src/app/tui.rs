use std::io::stdout;

use ratatui::crossterm;
use ratatui::crossterm::event::{
  self, DisableMouseCapture, EnableMouseCapture, Event,
  KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
  PushKeyboardEnhancementFlags,
};
use tokio_util::sync::CancellationToken;

use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::app::{App, AppEvent, EventSink};
use crate::persona_manager::Persona;
use crate::session_manager::session::Session;
use crate::slash_commands::SlashCommand;
use crate::tui::Tui;

struct TuiSink {
  tui: Tui,
  terminal: ratatui::DefaultTerminal,
}

impl TuiSink {
  fn redraw(&mut self) {
    self.terminal.draw(|frame| self.tui.draw(frame)).ok();
  }
}

#[async_trait::async_trait]
impl EventSink for TuiSink {
  fn on_event(&mut self, event: AppEvent) {
    match event {
      AppEvent::UserMessage { content } => {
        self.tui.add_message(content);
      }
      AppEvent::Token { text, reasoning } => {
        if reasoning {
          self.tui.ensure_reasoning_line();
        } else {
          self.tui.ensure_content_line();
        }
        self.tui.append_to_last_message(&text);
      }
      AppEvent::ToolCall { name, arguments } => {
        self.tui.add_tool_call(name, arguments);
      }
      AppEvent::ToolResult { result, error } => {
        self.tui.add_tool_result(result, error);
      }
      AppEvent::Processing { processing } => {
        self.tui.set_processing(processing);
      }
      AppEvent::Usage { usage } => {
        self.tui.set_usage(usage);
      }
      AppEvent::Error { message } => {
        self.tui.add_message(format!("Error: {message}"));
      }
    }
    self.redraw();
  }

  fn confirm(&mut self, message: &str) -> bool {
    self.tui.open_confirm_popup(message.to_string());
    self.redraw();
    while self.tui.confirm_popup_open() {
      if let Ok(Event::Key(key)) = event::read() {
        self.tui.handle_confirm_event(key.code);
      }
      self.redraw();
    }
    return self.tui.get_confirmation();
  }

  async fn read_input(&mut self) -> RuntimeResult<Option<String>> {
    loop {
      let input = self.tui.read_input().await;
      self.redraw();
      if let Some(text) = input {
        return Ok(Some(text));
      }
      if !self.tui.is_running() {
        return Ok(None);
      }
    }
  }

  fn on_exit(&mut self) {
    self.tui.quit();
  }

  fn on_help(&mut self, commands: Vec<SlashCommand>) {
    self.tui.open_help_popup(commands);
    self.redraw();
  }

  fn on_sessions(
    &mut self,
    sessions: Vec<Session>,
    current: Option<i64>,
  ) -> RuntimeResult<Option<i64>> {
    self.tui.open_sessions_popup(sessions, current);
    self.redraw();
    while self.tui.sessions_popup_open() {
      if let Ok(Event::Key(key)) = event::read() {
        self.tui.handle_sessions_event(key.code);
      }
      self.redraw();
    }
    let session_id = self.tui.get_selected_session();
    return Ok(session_id);
  }

  fn on_session_selection(&mut self, session: Session) {
    self.tui.update_messages(Some(&session));
    self.redraw();
  }

  fn on_personas(
    &mut self,
    personas: Vec<Persona>,
    current: String,
  ) -> RuntimeResult<Option<String>> {
    let current_persona = personas.iter().find(|p| p.name() == current);
    self.tui.open_personas_popup(&personas, current_persona);
    self.redraw();
    while self.tui.personas_popup_open() {
      if let Ok(Event::Key(key)) = event::read() {
        self.tui.handle_personas_event(key.code);
      }
      self.redraw();
    }
    let name = self.tui.get_selected_persona();
    return Ok(name);
  }

  fn on_persona_selection(&mut self, name: String) {
    self.tui.set_persona(&name);
    self.tui.update_messages(None);
    self.redraw();
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

    sink
      .terminal
      .draw(|frame| sink.tui.draw(frame))
      .map_err(|e| RuntimeError::Tui(e.to_string()))?;

    let cancel = CancellationToken::new();
    let result = self.harness_loop(&mut sink, &cancel).await;

    crossterm::execute!(stdout(), DisableMouseCapture)
      .map_err(|e| RuntimeError::Tui(e.to_string()))?;
    crossterm::execute!(stdout(), PopKeyboardEnhancementFlags)
      .map_err(|e| RuntimeError::Tui(e.to_string()))?;
    ratatui::restore();
    return result;
  }
}
