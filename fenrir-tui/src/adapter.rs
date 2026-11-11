use std::collections::VecDeque;
use std::io::stdout;
use std::time::Duration;

use anyhow::{Context, Result};
use ratatui::crossterm;
use ratatui::crossterm::event::{
  self, DisableMouseCapture, EnableMouseCapture, Event,
  KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
  PushKeyboardEnhancementFlags,
};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc::{self, Receiver, Sender};
use uuid::Uuid;

use fenrir_core::client::Client;
use fenrir_core::command::Command;
use fenrir_core::engine::broker::BrokerHandle;
use fenrir_core::engine::protocol::{
  PersonaInfo, ServerEvent, SessionSummary, SessionView, TurnOutcome,
};
use fenrir_core::provider_manager::Model;

use crate::tui::Tui;

const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const TICK_INTERVAL: Duration = Duration::from_millis(50);

enum Mode {
  Idle,
  Approval {
    session_id: Uuid,
    turn_id: Uuid,
    tool_call_id: String,
  },
  Sessions,
  Personas,
  Models,
}

struct Adapter {
  client: Client,
  tui: Tui,
  terminal: ratatui::DefaultTerminal,
  input: Receiver<Event>,
  input_open: bool,
  mode: Mode,
  pending_lines: VecDeque<String>,
  startup_models_request: Option<u64>,
  prediction_pending: bool,
  predict_after_turn: bool,
  predict_enabled: bool,
  exiting: bool,
}

pub async fn run(
  broker: &BrokerHandle,
  predict_enabled: bool,
  default_persona: &str,
) -> Result<()> {
  let cwd = std::env::current_dir()
    .map(|path| path.display().to_string())
    .unwrap_or_default();
  let client = Client::connect(broker, &cwd).await?;
  let (input_tx, input_rx) = mpsc::channel(64);
  spawn_input_task(input_tx);
  let mut adapter = Adapter {
    client,
    tui: Tui::new(&cwd),
    terminal: ratatui::init(),
    input: input_rx,
    input_open: true,
    mode: Mode::Idle,
    pending_lines: VecDeque::new(),
    startup_models_request: None,
    prediction_pending: false,
    predict_after_turn: false,
    predict_enabled,
    exiting: false,
  };

  let setup = crossterm::execute!(stdout(), EnableMouseCapture)
    .context("Failed to enable mouse capture")
    .and_then(|_| {
      crossterm::execute!(
        stdout(),
        PushKeyboardEnhancementFlags(
          KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        )
      )
      .context("Failed to enable keyboard enhancement flags")
    });
  if let Err(e) = setup {
    ratatui::restore();
    return Err(e);
  }

  adapter.tui.set_persona(default_persona);
  let request_id = adapter.client.list_models(None).await?;
  adapter.startup_models_request = Some(request_id);
  adapter.redraw();

  let result = adapter.run_loop().await;

  let _ = crossterm::execute!(stdout(), DisableMouseCapture);
  let _ = crossterm::execute!(stdout(), PopKeyboardEnhancementFlags);
  ratatui::restore();
  return result;
}

impl Adapter {
  async fn run_loop(&mut self) -> Result<()> {
    let mut sigint = signal(SignalKind::interrupt())
      .context("Failed to register SIGINT signal")?;
    let mut sigterm = signal(SignalKind::terminate())
      .context("Failed to register SIGTERM signal")?;
    let mut ticker = tokio::time::interval(TICK_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
      tokio::select! {
        event = self.client.recv() => {
          match event {
            Some(event) => {
              if self.handle_server_event(event).await? {
                break;
              }
            }
            None => break,
          }
        }
        event = self.input.recv(), if self.input_open => {
          match event {
            Some(event) => self.handle_input_event(event).await?,
            None => self.input_open = false,
          }
        }
        _ = sigint.recv() => {
          self.shutdown().await;
          break;
        }
        _ = sigterm.recv() => {
          self.shutdown().await;
          break;
        }
        _ = ticker.tick() => {}
      }

      self.process_mode().await?;
      self.flush_pending_lines().await?;
      self.process_cancel().await?;
      self.maybe_predict().await?;
      self.redraw();

      if self.exiting || !self.tui.is_running() {
        break;
      }
    }

    return Ok(());
  }

  async fn handle_input_event(&mut self, event: Event) -> Result<()> {
    let Some(input) = self.tui.handle_event(event).await else {
      return Ok(());
    };
    self.tui.clear_suggestion();
    self.prediction_pending = false;
    self.predict_after_turn = false;
    self.handle_line(&input).await?;
    return Ok(());
  }

  async fn handle_line(&mut self, line: &str) -> Result<()> {
    let command = match Command::parse(line) {
      Ok(command) => command,
      Err(e) => {
        self.tui.add_error(format!("Error: {e}"));
        return Ok(());
      }
    };

    let queueable = match command {
      None => true,
      Some(Command::Compact | Command::Review) => true,
      Some(_) => false,
    };
    if self.client.state().creating_session
      || (queueable
        && (self.client.state().active_turn().is_some()
          || self.client.state().is_pending()))
    {
      self.pending_lines.push_back(line.to_string());
      self.refresh_queue_display();
      return Ok(());
    }

    return match command {
      Some(command) => self.run_command(command).await,
      None => self.send_prompt(line).await,
    };
  }

  async fn send_prompt(&mut self, text: &str) -> Result<()> {
    self.client.send_prompt(text.to_string()).await?;
    return Ok(());
  }

  async fn run_command(&mut self, command: Command) -> Result<()> {
    if command == Command::Exit {
      self.shutdown().await;
      self.exiting = true;
      return Ok(());
    }

    self.client.run_command(command).await?;
    return Ok(());
  }

  async fn handle_server_event(&mut self, event: ServerEvent) -> Result<bool> {
    match event {
      ServerEvent::Connected { .. } => {}
      ServerEvent::SessionCreated { session, .. } => {
        self.show_session(&session);
      }
      ServerEvent::SessionLoaded { session, .. } => {
        self.show_session(&session);
      }
      ServerEvent::SessionsListed { sessions, .. } => {
        self.open_sessions(sessions);
      }
      ServerEvent::PersonasListed {
        personas, current, ..
      } => {
        self.open_personas(personas, current);
      }
      ServerEvent::ModelsListed {
        request_id,
        models,
        current,
      } => {
        if self.startup_models_request == Some(request_id) {
          self.startup_models_request = None;
          if let Some(model) = current {
            self.tui.set_model(&model.label());
          }
        } else {
          self.open_models(models, current);
        }
      }
      ServerEvent::PersonaUpdated { persona, .. } => {
        self.tui.set_persona(&persona);
      }
      ServerEvent::ModelUpdated { model, .. } => {
        self.tui.set_model(&model.label());
      }
      ServerEvent::CommandCatalog { commands, .. } => {
        self.tui.open_help_popup(commands);
      }
      ServerEvent::UserMessage { content, .. } => {
        self.tui.clear_suggestion();
        self.tui.add_message(content);
        self.prediction_pending = false;
        self.predict_after_turn = false;
      }
      ServerEvent::TurnStarted { .. } => {
        self.prediction_pending = false;
        self.predict_after_turn = false;
        self.tui.set_processing(true);
      }
      ServerEvent::Token {
        text, reasoning, ..
      } => {
        self.tui.clear_retrying();
        if reasoning {
          self.tui.ensure_reasoning_line();
        } else {
          self.tui.ensure_content_line();
        }
        self.tui.append_to_last_message(&text);
      }
      ServerEvent::ToolCall {
        id,
        name,
        arguments,
        preview,
        ..
      } => {
        self.tui.clear_retrying();
        self.tui.add_tool_call(id, name, arguments, preview);
      }
      ServerEvent::ToolApprovalRequest { .. } => {}
      ServerEvent::ToolApprovalResult { .. } => {}
      ServerEvent::ToolResult {
        id, result, error, ..
      } => {
        self.tui.add_tool_result(id, result, error);
      }
      ServerEvent::Usage {
        usage,
        context_window,
        ..
      } => {
        self.tui.set_usage(usage, context_window);
      }
      ServerEvent::Subagents { delta, .. } => {
        self.tui.update_subagents(delta);
      }
      ServerEvent::Compacted { summary, .. } => {
        self.tui.add_summary(summary);
      }
      ServerEvent::Prediction { text, .. } => {
        if self.prediction_pending {
          self.prediction_pending = false;
          if self.client.state().active_turn().is_none()
            && !self.client.state().is_pending()
            && matches!(self.mode, Mode::Idle)
            && self.tui.input_is_empty()
            && !self.tui.has_suggestion()
          {
            self.tui.set_suggestion(text);
          }
        }
      }
      ServerEvent::TitleUpdated { .. } => {}
      ServerEvent::QueueUpdated { .. } => {
        self.refresh_queue_display();
      }
      ServerEvent::TurnFinished {
        correlation,
        outcome,
      } => {
        self.tui.set_processing(false);
        self.tui.reset_subagents();
        self.predict_after_turn =
          matches!(outcome, TurnOutcome::Completed { .. });
        if let Mode::Approval {
          session_id,
          turn_id,
          ..
        } = &self.mode
          && *session_id == correlation.session_id
          && *turn_id == correlation.turn_id
        {
          self.tui.close_confirm_popup();
          self.mode = Mode::Idle;
        }
      }
      ServerEvent::Retrying {
        attempt,
        max_attempts,
        delay_seconds,
        ..
      } => {
        self.tui.set_retrying(attempt, max_attempts, delay_seconds);
      }
      ServerEvent::Error { message, .. } => {
        self.tui.clear_retrying();
        self.tui.add_error(format!("Error: {message}"));
      }
      ServerEvent::SessionStopped { .. } => {
        self.prediction_pending = false;
        self.tui.set_processing(false);
        self.tui.reset_subagents();
        self.refresh_queue_display();
        if let Mode::Approval { .. } = self.mode {
          self.tui.close_confirm_popup();
          self.mode = Mode::Idle;
        }
      }
      ServerEvent::Exited => {
        self.exiting = true;
        return Ok(true);
      }
    }
    self.flush_pending_lines().await?;
    return Ok(false);
  }

  async fn process_mode(&mut self) -> Result<()> {
    if matches!(self.mode, Mode::Sessions) && !self.tui.sessions_popup_open() {
      self.mode = Mode::Idle;
      if let Some(session_id) = self.tui.get_selected_session() {
        self.client.load_session(session_id).await?;
      }
    } else if matches!(self.mode, Mode::Personas)
      && !self.tui.personas_popup_open()
    {
      self.mode = Mode::Idle;
      if let Some(name) = self.tui.get_selected_persona() {
        self.client.set_persona(name).await?;
      }
    } else if matches!(self.mode, Mode::Models) && !self.tui.models_popup_open()
    {
      self.mode = Mode::Idle;
      if let Some(model) = self.tui.get_selected_model() {
        self.client.set_model(model).await?;
      }
    } else if matches!(self.mode, Mode::Approval { .. })
      && !self.tui.confirm_popup_open()
    {
      let Mode::Approval {
        session_id,
        turn_id,
        tool_call_id,
      } = std::mem::replace(&mut self.mode, Mode::Idle)
      else {
        unreachable!();
      };
      let approved = self.tui.get_confirmation();
      self
        .client
        .approve_tool(session_id, turn_id, tool_call_id, approved)
        .await?;
    }

    self.open_next_approval();
    return Ok(());
  }

  async fn process_cancel(&mut self) -> Result<()> {
    if !self.tui.take_cancel_request() {
      return Ok(());
    }
    let Some(turn_id) = self.client.state().active_turn() else {
      return Ok(());
    };
    self.client.cancel(Some(turn_id)).await?;
    return Ok(());
  }

  async fn maybe_predict(&mut self) -> Result<()> {
    if !self.predict_enabled
      || !self.predict_after_turn
      || self.prediction_pending
      || self.client.state().active_turn().is_some()
      || self.client.state().is_pending()
      || self.has_queued_messages()
      || !matches!(self.mode, Mode::Idle)
      || !self.tui.input_is_empty()
      || self.tui.has_suggestion()
    {
      return Ok(());
    }
    let Some(session_id) = self.client.state().visible_session() else {
      return Ok(());
    };
    self.predict_after_turn = false;
    self.prediction_pending = true;
    self.client.predict(session_id).await?;
    return Ok(());
  }

  async fn flush_pending_lines(&mut self) -> Result<()> {
    while !self.client.state().creating_session
      && !self.client.state().is_pending()
      && !self.exiting
      && self.client.state().active_turn().is_none()
      && matches!(self.mode, Mode::Idle)
    {
      let Some(line) = self.pending_lines.pop_front() else {
        break;
      };
      self.refresh_queue_display();
      self.handle_line(&line).await?;
    }
    return Ok(());
  }

  fn show_session(&mut self, session: &SessionView) {
    self.tui.set_persona(&session.persona);
    if let Some(model) = &session.model {
      self.tui.set_model(&model.label());
    }
    self.tui.update_messages(Some(&session.messages));
    self.refresh_queue_display();
    self.prediction_pending = false;
    self.predict_after_turn = false;
  }

  fn refresh_queue_display(&mut self) {
    let mut messages: Vec<String> = self
      .client
      .state()
      .queued_messages()
      .iter()
      .map(|message| message.content.clone())
      .collect();
    messages.extend(self.pending_lines.iter().cloned());
    self.tui.update_queued_messages(messages);
  }

  fn has_queued_messages(&self) -> bool {
    return !self.client.state().queued_messages().is_empty()
      || !self.pending_lines.is_empty();
  }

  fn open_sessions(&mut self, sessions: Vec<SessionSummary>) {
    let active_session = self.client.state().visible_session();
    self.tui.open_sessions_popup(sessions, active_session);
    self.mode = Mode::Sessions;
  }

  fn open_personas(
    &mut self,
    personas: Vec<PersonaInfo>,
    current: Option<String>,
  ) {
    self.tui.open_personas_popup(&personas, current.as_deref());
    self.mode = Mode::Personas;
  }

  fn open_models(&mut self, models: Vec<Model>, current: Option<Model>) {
    self.tui.open_models_popup(models, current);
    self.mode = Mode::Models;
  }

  fn open_next_approval(&mut self) {
    if !matches!(self.mode, Mode::Idle)
      || self.tui.confirm_popup_open()
      || self.exiting
    {
      return;
    }
    let Some(approval) = self.client.take_next_approval() else {
      return;
    };
    self.tui.open_confirm_popup(approval.message);
    self.mode = Mode::Approval {
      session_id: approval.session_id,
      turn_id: approval.turn_id,
      tool_call_id: approval.tool_call_id,
    };
  }

  async fn shutdown(&mut self) {
    self.tui.quit();
    let _ = self.client.shutdown().await;
  }

  fn redraw(&mut self) {
    self.terminal.draw(|frame| self.tui.draw(frame)).ok();
  }
}

fn spawn_input_task(tx: Sender<Event>) {
  std::thread::spawn(move || {
    loop {
      match event::poll(INPUT_POLL_INTERVAL) {
        Ok(true) => match event::read() {
          Ok(event) => {
            if tx.blocking_send(event).is_err() {
              break;
            }
          }
          Err(_) => break,
        },
        Ok(false) => {}
        Err(_) => break,
      }
    }
  });
}
