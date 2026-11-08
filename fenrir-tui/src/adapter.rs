use std::collections::{HashSet, VecDeque};
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
use tokio::sync::mpsc;
use uuid::Uuid;

use fenrir_core::command::Command;
use fenrir_core::engine::broker::{BrokerHandle, ClientHandle};
use fenrir_core::engine::protocol::{
  ClientRequest, PersonaInfo, PromptMode, ServerEvent, SessionSummary,
  SessionView, TurnOutcome,
};
use fenrir_core::persona_manager::{DEFAULT_PERSONA, REVIEWER_PERSONA};
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

struct PendingApproval {
  session_id: Uuid,
  turn_id: Uuid,
  tool_call_id: String,
  message: String,
}

struct Adapter {
  client: ClientHandle,
  tui: Tui,
  terminal: ratatui::DefaultTerminal,
  input: mpsc::Receiver<Event>,
  input_open: bool,
  mode: Mode,
  approvals: VecDeque<PendingApproval>,
  pending_lines: VecDeque<String>,
  creating_session: bool,
  active_session: Option<Uuid>,
  active_turn: Option<Uuid>,
  busy: bool,
  next_request_id: u64,
  pending_request_ids: HashSet<u64>,
  startup_models_request: Option<u64>,
  prediction_pending: bool,
  predict_after_turn: bool,
  predict_enabled: bool,
  review_pending: bool,
  server_queue: Vec<String>,
  exiting: bool,
}

pub async fn run(broker: &BrokerHandle, predict_enabled: bool) -> Result<()> {
  let cwd = std::env::current_dir()
    .map(|path| path.display().to_string())
    .unwrap_or_default();
  let client = broker.connect(&cwd).await?;
  let (input_tx, input_rx) = mpsc::channel(64);
  spawn_input_task(input_tx);
  let mut adapter = Adapter {
    client,
    tui: Tui::new(&cwd),
    terminal: ratatui::init(),
    input: input_rx,
    input_open: true,
    mode: Mode::Idle,
    approvals: VecDeque::new(),
    pending_lines: VecDeque::new(),
    creating_session: false,
    active_session: None,
    active_turn: None,
    busy: false,
    next_request_id: 1,
    pending_request_ids: HashSet::new(),
    startup_models_request: None,
    prediction_pending: false,
    predict_after_turn: false,
    predict_enabled,
    review_pending: false,
    server_queue: Vec::new(),
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

  adapter.tui.set_persona(DEFAULT_PERSONA.name());
  let request_id = adapter.take_request_id();
  adapter.startup_models_request = Some(request_id);
  adapter
    .client
    .send(ClientRequest::ListModels {
      request_id,
      session_id: None,
    })
    .await?;
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
        event = self.client.events.recv() => {
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
        self.tui.add_message(format!("Error: {e}"));
        return Ok(());
      }
    };

    if self.creating_session
      || (command.is_some() && (self.active_turn.is_some() || self.busy))
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
    let request_id = self.take_request_id();
    self.pending_request_ids.insert(request_id);
    if self.active_session.is_none() {
      self.creating_session = true;
    }
    self
      .client
      .send(ClientRequest::SendPrompt {
        request_id,
        session_id: self.active_session,
        mode: PromptMode::Normal,
        text: Some(text.to_string()),
      })
      .await?;
    return Ok(());
  }

  async fn run_command(&mut self, command: Command) -> Result<()> {
    if command == Command::Exit {
      self.shutdown().await;
      self.exiting = true;
      return Ok(());
    }

    if command == Command::Review {
      self.review_pending = true;
      self.tui.set_persona(REVIEWER_PERSONA.name());
    }

    let request_id = self.take_request_id();
    self.pending_request_ids.insert(request_id);
    self.busy = true;
    self
      .client
      .send(ClientRequest::RunCommand {
        request_id,
        session_id: self.active_session,
        command,
      })
      .await?;
    return Ok(());
  }

  async fn handle_server_event(&mut self, event: ServerEvent) -> Result<bool> {
    match event {
      ServerEvent::Connected { .. } => {}
      ServerEvent::SessionCreated {
        request_id,
        session,
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        self.creating_session = false;
        self.switch_session(&session).await?;
        if self.review_pending {
          self.tui.set_persona(REVIEWER_PERSONA.name());
        }
      }
      ServerEvent::SessionLoaded {
        request_id,
        session,
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        self.switch_session(&session).await?;
      }
      ServerEvent::SessionsListed {
        request_id,
        sessions,
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        self.open_sessions(sessions);
      }
      ServerEvent::PersonasListed {
        request_id,
        personas,
        current,
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        self.open_personas(personas, current);
      }
      ServerEvent::ModelsListed {
        request_id,
        models,
        current,
      } => {
        self.pending_request_ids.remove(&request_id);
        if self.startup_models_request == Some(request_id) {
          self.startup_models_request = None;
          if let Some(model) = current {
            self.tui.set_model(&model.label());
          }
        } else {
          self.busy = false;
          self.open_models(models, current);
        }
      }
      ServerEvent::PersonaUpdated {
        request_id,
        persona,
        ..
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        self.tui.set_persona(&persona);
      }
      ServerEvent::ModelUpdated {
        request_id, model, ..
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        self.tui.set_model(&model.label());
      }
      ServerEvent::CommandCatalog {
        request_id,
        commands,
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        self.tui.open_help_popup(commands);
      }
      ServerEvent::UserMessage {
        correlation,
        content,
        ..
      } => {
        if self.active_session != Some(correlation.session_id) {
          return Ok(false);
        }
        self.tui.clear_suggestion();
        self.tui.add_message(content);
        self.prediction_pending = false;
        self.predict_after_turn = false;
      }
      ServerEvent::TurnStarted { correlation, .. } => {
        if self.active_session.is_none() {
          self.active_session = Some(correlation.session_id);
        }
        if self.active_session != Some(correlation.session_id) {
          return Ok(false);
        }
        self.active_turn = Some(correlation.turn_id);
        self.busy = false;
        self.review_pending = false;
        self.prediction_pending = false;
        self.predict_after_turn = false;
        self.tui.set_processing(true);
      }
      ServerEvent::Token {
        correlation,
        text,
        reasoning,
      } => {
        if self.active_session != Some(correlation.session_id) {
          return Ok(false);
        }
        if reasoning {
          self.tui.ensure_reasoning_line();
        } else {
          self.tui.ensure_content_line();
        }
        self.tui.append_to_last_message(&text);
      }
      ServerEvent::ToolCall {
        correlation,
        id,
        name,
        arguments,
        preview,
      } => {
        if self.active_session != Some(correlation.session_id) {
          return Ok(false);
        }
        self.tui.add_tool_call(id, name, arguments, preview);
      }
      ServerEvent::ToolApprovalRequest {
        correlation,
        tool_call_id,
        message,
        ..
      } => {
        if self.active_session != Some(correlation.session_id) {
          return Ok(false);
        }
        self.approvals.push_back(PendingApproval {
          session_id: correlation.session_id,
          turn_id: correlation.turn_id,
          tool_call_id,
          message,
        });
      }
      ServerEvent::ToolApprovalResult { .. } => {}
      ServerEvent::ToolResult {
        correlation,
        id,
        result,
        error,
      } => {
        if self.active_session != Some(correlation.session_id) {
          return Ok(false);
        }
        self.tui.add_tool_result(id, result, error);
      }
      ServerEvent::Usage {
        correlation,
        usage,
        context_window,
      } => {
        if self.active_session != Some(correlation.session_id) {
          return Ok(false);
        }
        self.tui.set_usage(usage, context_window);
      }
      ServerEvent::Subagents { correlation, delta } => {
        if self.active_session != Some(correlation.session_id) {
          return Ok(false);
        }
        self.tui.update_subagents(delta);
      }
      ServerEvent::Compacted {
        session_id,
        summary,
      } => {
        if self.active_session != Some(session_id) {
          return Ok(false);
        }
        self.tui.add_summary(summary);
      }
      ServerEvent::Prediction { session_id, text } => {
        if self.active_session == Some(session_id) && self.prediction_pending {
          self.prediction_pending = false;
          if self.active_turn.is_none()
            && !self.busy
            && matches!(self.mode, Mode::Idle)
            && self.tui.input_is_empty()
            && !self.tui.has_suggestion()
          {
            self.tui.set_suggestion(text);
          }
        }
      }
      ServerEvent::TitleUpdated { .. } => {}
      ServerEvent::QueueUpdated {
        session_id,
        messages,
      } => {
        if self.active_session == Some(session_id) {
          self.server_queue = messages
            .into_iter()
            .map(|message| message.content)
            .collect();
          self.refresh_queue_display();
        }
      }
      ServerEvent::TurnFinished {
        correlation,
        outcome,
      } => {
        if self.active_session != Some(correlation.session_id) {
          return Ok(false);
        }
        if self.active_turn == Some(correlation.turn_id) {
          self.active_turn = None;
        }
        self.busy = false;
        self.review_pending = false;
        self.tui.set_processing(false);
        self.tui.reset_subagents();
        self.clear_approvals(correlation.session_id, correlation.turn_id);
        self.predict_after_turn =
          matches!(outcome, TurnOutcome::Completed { .. });
      }
      ServerEvent::Error {
        request_id,
        message,
        ..
      } => {
        if let Some(request_id) = request_id {
          if self.pending_request_ids.remove(&request_id) {
            self.busy = false;
          }
          if self.creating_session && self.active_session.is_none() {
            self.creating_session = false;
          }
        }
        self.review_pending = false;
        self.tui.add_message(format!("Error: {message}"));
      }
      ServerEvent::SessionStopped { session_id } => {
        if self.active_session == Some(session_id) {
          self.active_session = None;
          self.active_turn = None;
          self.prediction_pending = false;
          self.tui.set_processing(false);
          self.tui.reset_subagents();
          self.server_queue.clear();
          self.refresh_queue_display();
        }
        self
          .approvals
          .retain(|approval| approval.session_id != session_id);
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
        let request_id = self.take_request_id();
        self.pending_request_ids.insert(request_id);
        self.busy = true;
        self
          .client
          .send(ClientRequest::LoadSession {
            request_id,
            session_id,
          })
          .await?;
      }
    } else if matches!(self.mode, Mode::Personas)
      && !self.tui.personas_popup_open()
    {
      self.mode = Mode::Idle;
      if let Some(name) = self.tui.get_selected_persona() {
        let request_id = self.take_request_id();
        self.pending_request_ids.insert(request_id);
        self.busy = true;
        self
          .client
          .send(ClientRequest::SetPersona {
            request_id,
            session_id: self.active_session,
            name,
          })
          .await?;
      }
    } else if matches!(self.mode, Mode::Models) && !self.tui.models_popup_open()
    {
      self.mode = Mode::Idle;
      if let Some(model) = self.tui.get_selected_model() {
        let request_id = self.take_request_id();
        self.pending_request_ids.insert(request_id);
        self.busy = true;
        self
          .client
          .send(ClientRequest::SetModel {
            request_id,
            session_id: self.active_session,
            model,
          })
          .await?;
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
        .send(ClientRequest::ApproveTool {
          session_id,
          turn_id,
          tool_call_id,
          approved,
        })
        .await?;
    }

    self.open_next_approval();
    return Ok(());
  }

  async fn process_cancel(&mut self) -> Result<()> {
    if !self.tui.take_cancel_request() {
      return Ok(());
    }
    let (Some(session_id), Some(turn_id)) =
      (self.active_session, self.active_turn)
    else {
      return Ok(());
    };
    self
      .client
      .send(ClientRequest::Cancel {
        session_id,
        turn_id: Some(turn_id),
      })
      .await?;
    return Ok(());
  }

  async fn maybe_predict(&mut self) -> Result<()> {
    if !self.predict_enabled
      || !self.predict_after_turn
      || self.prediction_pending
      || self.active_turn.is_some()
      || self.busy
      || self.has_queued_messages()
      || !matches!(self.mode, Mode::Idle)
      || !self.tui.input_is_empty()
      || self.tui.has_suggestion()
    {
      return Ok(());
    }
    let Some(session_id) = self.active_session else {
      return Ok(());
    };
    self.predict_after_turn = false;
    self.prediction_pending = true;
    self
      .client
      .send(ClientRequest::Predict { session_id })
      .await?;
    return Ok(());
  }

  async fn flush_pending_lines(&mut self) -> Result<()> {
    while !self.creating_session
      && !self.busy
      && !self.exiting
      && self.active_turn.is_none()
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

  async fn switch_session(&mut self, session: &SessionView) -> Result<()> {
    let new_id = session.summary.id;
    if let Some(old_id) = self.active_session
      && old_id != new_id
    {
      let _ = self
        .client
        .send(ClientRequest::Unsubscribe { session_id: old_id })
        .await;
    }
    self.active_session = Some(new_id);
    self.active_turn = None;
    self.server_queue.clear();
    self.show_session(session);
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
    let mut messages = self.server_queue.clone();
    messages.extend(self.pending_lines.iter().cloned());
    self.tui.update_queued_messages(messages);
  }

  fn has_queued_messages(&self) -> bool {
    return !self.server_queue.is_empty() || !self.pending_lines.is_empty();
  }

  fn open_sessions(&mut self, sessions: Vec<SessionSummary>) {
    self.tui.open_sessions_popup(sessions, self.active_session);
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
    let Some(approval) = self.approvals.pop_front() else {
      return;
    };
    self.tui.open_confirm_popup(approval.message);
    self.mode = Mode::Approval {
      session_id: approval.session_id,
      turn_id: approval.turn_id,
      tool_call_id: approval.tool_call_id,
    };
  }

  fn clear_approvals(&mut self, session_id: Uuid, turn_id: Uuid) {
    self.approvals.retain(|approval| {
      approval.session_id != session_id || approval.turn_id != turn_id
    });
    if let Mode::Approval {
      session_id: current_session,
      turn_id: current_turn,
      ..
    } = &self.mode
      && *current_session == session_id
      && *current_turn == turn_id
    {
      self.tui.close_confirm_popup();
      self.mode = Mode::Idle;
    }
  }

  async fn shutdown(&mut self) {
    self.tui.quit();
    let _ = self.client.send(ClientRequest::Shutdown).await;
  }

  fn redraw(&mut self) {
    self.terminal.draw(|frame| self.tui.draw(frame)).ok();
  }

  fn take_request_id(&mut self) -> u64 {
    let request_id = self.next_request_id;
    self.next_request_id += 1;
    return request_id;
  }
}

fn spawn_input_task(tx: mpsc::Sender<Event>) {
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
