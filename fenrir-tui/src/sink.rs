use std::io::stdout;
use std::time::Duration;

use anyhow::{Context, Result};
use ratatui::crossterm;
use ratatui::crossterm::event::{
  self, DisableMouseCapture, EnableMouseCapture, Event,
  KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
  PushKeyboardEnhancementFlags,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use fenrir_core::app::{App, AppEvent, ApprovalChannel, EventSink, InputEvent};
use fenrir_core::engine::protocol::{CommandInfo, SessionSummary};
use fenrir_core::persona_manager::Persona;
use fenrir_core::provider_manager::Model;
use fenrir_core::session_manager::ToolApprovalStatus;
use fenrir_core::session_manager::session::Session;

use crate::tui::Tui;

const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(50);

struct TuiSink {
  tui: Tui,
  terminal: ratatui::DefaultTerminal,
  approvals: Option<ApprovalChannel>,
}

impl TuiSink {
  fn redraw(&mut self) {
    self.terminal.draw(|frame| self.tui.draw(frame)).ok();
  }
}

#[async_trait::async_trait]
impl EventSink for TuiSink {
  async fn on_event(&mut self, event: AppEvent) {
    match event {
      AppEvent::UserMessage { content } => {
        self.tui.clear_suggestion();
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
      AppEvent::ToolCall {
        id,
        name,
        arguments,
        preview,
      } => {
        self.tui.add_tool_call(id, name, arguments, preview);
      }
      AppEvent::ToolApprovalRequest {
        tool_call_id,
        tool_name: _,
        message,
      } => {
        self.tui.open_confirm_popup(message);
        self.redraw();
        while self.tui.confirm_popup_open() {
          if let Ok(Event::Key(key)) = event::read() {
            self.tui.handle_confirm_event(key.code);
          }
          self.redraw();
        }
        let status = if self.tui.get_confirmation() {
          ToolApprovalStatus::Approved
        } else {
          ToolApprovalStatus::Denied
        };
        if let Some(channel) = &self.approvals {
          channel.decide(&tool_call_id, status);
        }
      }
      AppEvent::ToolApprovalResult {
        tool_call_id: _,
        status: _,
      } => {}
      AppEvent::ToolResult { id, result, error } => {
        self.tui.add_tool_result(id, result, error);
      }
      AppEvent::Processing { processing } => {
        self.tui.set_processing(processing);
        if !processing {
          self.tui.reset_subagents();
        }
      }
      AppEvent::Usage {
        usage,
        context_window,
      } => {
        self.tui.set_usage(usage, context_window);
      }
      AppEvent::Subagents { delta } => {
        self.tui.update_subagents(delta);
      }
      AppEvent::Compacted { summary } => {
        self.tui.add_summary(summary);
      }
      AppEvent::Prediction { text } => {
        self.tui.set_suggestion(text);
      }
      AppEvent::Error { message } => {
        self.tui.add_message(format!("Error: {message}"));
      }
      AppEvent::QueueUpdated { messages } => {
        self.tui.update_queued_messages(messages);
      }
    }
    self.redraw();
  }

  async fn poll_input(&mut self) -> Result<InputEvent> {
    if !self.tui.is_running() {
      return Ok(InputEvent::Exit);
    }
    if !event::poll(INPUT_POLL_INTERVAL)
      .context("Failed to poll terminal events")?
    {
      return Ok(InputEvent::Pending);
    }
    let event = event::read().context("Failed to read terminal event")?;
    let input = self.tui.handle_event(event).await;
    self.redraw();
    if !self.tui.is_running() {
      return Ok(InputEvent::Exit);
    }
    return Ok(match input {
      Some(text) => InputEvent::Input(text),
      None => InputEvent::Pending,
    });
  }

  fn wants_prediction(&self) -> bool {
    return self.tui.is_running()
      && self.tui.input_is_empty()
      && !self.tui.has_suggestion();
  }

  async fn poll_events(&mut self) -> Option<String> {
    let input = if event::poll(Duration::from_millis(10)).unwrap_or(false)
      && let Ok(e) = event::read()
      && let Some(input) = self.tui.handle_event(e).await
    {
      Some(input)
    } else {
      None
    };
    self.redraw();
    return input;
  }

  async fn on_exit(&mut self) {
    self.tui.quit();
  }

  async fn on_help(&mut self, commands: Vec<CommandInfo>) {
    self.tui.open_help_popup(commands);
    self.redraw();
  }

  async fn on_sessions(
    &mut self,
    sessions: Vec<SessionSummary>,
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

  async fn on_session_selection(&mut self, session: &Session) {
    self
      .tui
      .set_persona(session.persona_manager.current().name());
    if let Some(model) = &session.model {
      self.tui.set_model(&model.label());
    }
    self.tui.update_messages(Some(session));
    self.redraw();
  }

  async fn on_personas(
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

  async fn on_persona_selection(&mut self, name: String) {
    self.tui.set_persona(&name);
    self.redraw();
  }

  async fn on_new_session(
    &mut self,
    persona_name: String,
    model: Option<Model>,
  ) {
    self.tui.set_persona(&persona_name);
    if let Some(model) = model {
      self.tui.set_model(&model.label());
    }
    self.tui.update_messages(None);
    self.redraw();
  }

  async fn on_models(
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

  async fn on_model_selection(&mut self, model: Model) {
    self.tui.set_model(&model.label());
    self.redraw();
  }

  async fn set_approval_channel(&mut self, channel: Option<ApprovalChannel>) {
    self.approvals = channel;
  }

  async fn set_cancel_token(&mut self, token: CancellationToken) {
    self.tui.set_cancel_token(token);
  }
}

pub async fn run(app: &mut App) -> Result<()> {
  let cwd = std::env::current_dir()
    .map(|p| p.display().to_string())
    .unwrap_or_default();
  let mut sink = TuiSink {
    tui: Tui::new(&cwd),
    terminal: ratatui::init(),
    approvals: None,
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

  let persona_name = app.sessions.persona_name(None);
  sink.tui.set_persona(&persona_name);

  let model = app.sessions.current_model(None);
  if let Some(model) = model {
    sink.tui.set_model(&model.label());
  }

  sink
    .terminal
    .draw(|frame| sink.tui.draw(frame))
    .context("Failed to draw the TUI")?;

  let result = app.run(&mut sink, &cwd).await;

  crossterm::execute!(stdout(), DisableMouseCapture)
    .context("Failed to disable mouse capture")?;
  crossterm::execute!(stdout(), PopKeyboardEnhancementFlags)
    .context("Failed to restore keyboard enhancement flags")?;
  ratatui::restore();
  return result;
}
