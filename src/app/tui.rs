use std::collections::VecDeque;
use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use ratatui::crossterm;
use ratatui::crossterm::event::{
  self, DisableMouseCapture, EnableMouseCapture, Event,
  KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
  PushKeyboardEnhancementFlags,
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::agent_manager::AgentManager;
use crate::app::{App, AppEvent, EventSink};
use crate::persona_manager::Persona;
use crate::provider_manager::Model;
use crate::session_manager::session::Session;
use crate::slash_commands::SlashCommand;
use crate::tui::Tui;

struct TuiSink {
  tui: Tui,
  terminal: ratatui::DefaultTerminal,
  message_queue: VecDeque<String>,
}

impl TuiSink {
  fn redraw(&mut self) {
    self.terminal.draw(|frame| self.tui.draw(frame)).ok();
  }

  fn clear_message_queue(&mut self) {
    self.message_queue.clear();
    self.tui.update_queued_messages(Vec::new());
  }

  fn add_to_message_queue(&mut self, text: String) {
    self.message_queue.push_back(text);
    self
      .tui
      .update_queued_messages(self.message_queue.make_contiguous().to_vec());
  }

  fn get_message_from_queue(&mut self) -> Option<String> {
    let message = self.message_queue.pop_front();
    self
      .tui
      .update_queued_messages(self.message_queue.make_contiguous().to_vec());
    return message;
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
      AppEvent::SubagentStarted { task } => {
        self.tui.add_subagent_launch(task);
      }
      AppEvent::ToolResult { result, error } => {
        self.tui.add_tool_result(result, error);
      }
      AppEvent::Processing { processing } => {
        self.tui.set_processing(processing);
        if !processing {
          self.tui.reset_subagents();
        }
      }
      AppEvent::Usage { usage } => {
        self.tui.set_usage(usage);
      }
      AppEvent::Subagents { delta } => {
        self.tui.update_subagents(delta);
      }
      AppEvent::Compacted { summary } => {
        self.tui.add_summary(summary);
      }
      AppEvent::Error { message } => {
        self.tui.add_message(format!("Error: {message}"));
      }
      AppEvent::ToolConfirmation { message, reply } => {
        let confirmed = self.confirm(&message);
        reply.send(confirmed).ok();
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

  async fn read_input(&mut self) -> Result<Option<String>> {
    loop {
      if let Some(text) = self.get_message_from_queue() {
        return Ok(Some(text));
      }
      let input = self.tui.read_input().await;
      self.redraw();
      if let Some(text) = input {
        self.add_to_message_queue(text);
      }
      if !self.tui.is_running() {
        return Ok(None);
      }
    }
  }

  async fn poll_events(&mut self) {
    if event::poll(Duration::from_millis(10)).unwrap_or(false)
      && let Ok(e) = event::read()
      && let Some(input) = self.tui.handle_event(e).await
    {
      self.add_to_message_queue(input);
    }
    self.redraw();
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
    current: Option<Uuid>,
  ) -> Result<Option<Uuid>> {
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
    self.clear_message_queue();
    self.tui.update_messages(Some(&session));
    self.redraw();
  }

  fn on_personas(
    &mut self,
    personas: Vec<Persona>,
    current: String,
  ) -> Result<Option<String>> {
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
    self.clear_message_queue();
    self.tui.set_persona(&name);
    self.tui.update_messages(None);
    self.redraw();
  }

  fn on_models(
    &mut self,
    models: Vec<Model>,
    current: Option<Model>,
  ) -> Result<Option<Model>> {
    self.tui.open_models_popup(models, current);
    self.redraw();
    while self.tui.models_popup_open() {
      if let Ok(Event::Key(key)) = event::read() {
        self.tui.handle_models_event(key.code);
      }
      self.redraw();
    }
    let model = self.tui.get_selected_model();
    return Ok(model);
  }

  fn on_model_selection(&mut self, model: Model) {
    self.tui.set_model(&model.label());
    self.redraw();
  }

  fn set_cancel_token(&mut self, token: CancellationToken) {
    self.tui.set_cancel_token(token);
  }
}

impl App {
  pub async fn run_tui(app: &Arc<Mutex<App>>) -> Result<String> {
    let mut sink = TuiSink {
      tui: Tui::new(),
      terminal: ratatui::init(),
      message_queue: VecDeque::new(),
    };
    crossterm::execute!(stdout(), EnableMouseCapture)
      .context("Failed to enable mouse capture")?;
    crossterm::execute!(
      stdout(),
      PushKeyboardEnhancementFlags(
        KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
      )
    )
    .context("Failed to enable keyboard enhancement flags")?;

    let persona_name = {
      let app_locked = app.lock().await;
      app_locked
        .agent_manager
        .current
        .persona_manager
        .current()
        .name()
        .to_string()
    };
    sink.tui.set_persona(&persona_name);

    let model = {
      let app_locked = app.lock().await;
      app_locked.agent_manager.provider_manager.active_model()
    };
    if let Some(model) = model {
      sink.tui.set_model(&model.label());
    }

    sink
      .terminal
      .draw(|frame| sink.tui.draw(frame))
      .context("Failed to draw the TUI")?;

    let result = AgentManager::harness_loop(app, &mut sink).await;

    crossterm::execute!(stdout(), DisableMouseCapture)
      .context("Failed to disable mouse capture")?;
    crossterm::execute!(stdout(), PopKeyboardEnhancementFlags)
      .context("Failed to restore keyboard enhancement flags")?;
    ratatui::restore();
    return result;
  }
}
