use std::future::Future;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

use anyhow::{Context, Result};
use futures::future::LocalBoxFuture;
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::agent::{AgentContext, run_compact, run_turn};
use crate::app::App;
use crate::app::AppEvent;
use crate::app::ApprovalChannel;
use crate::app::EventSink;
use crate::app::InputEvent;
use crate::command::{Command, Effect};
use crate::engine::protocol::{
  ApprovalDecision, PromptMode, ServerEvent, SessionEvent, TurnOutcome,
};
use crate::engine::repo_actor::RepoHandle;
use crate::persona_manager::{PersonaManager, REVIEWER_PERSONA};
use crate::session_manager::{SessionContext, ToolApprovalStatus};
use crate::vlog;

const UI_TICK: Duration = Duration::from_millis(16);
const PREDICTION_YIELD: Duration = Duration::from_millis(1);

async fn persist_queued_input(
  sink: &mut impl EventSink,
  repository: &RepoHandle,
  session_id: Uuid,
) {
  let Some(input) = sink.poll_events().await else {
    return;
  };

  if let Err(e) = repository.insert_queued_message(session_id, &input).await {
    vlog!("Failed to queue message: {e}");
    return;
  }

  match repository.load_queued_messages(session_id).await {
    Ok(messages) => {
      let messages = messages
        .into_iter()
        .map(|message| message.content)
        .collect();
      sink.on_event(AppEvent::QueueUpdated { messages }).await;
    }
    Err(e) => {
      vlog!("Failed to load queued messages: {e}");
    }
  }
}

impl App {
  pub async fn run(
    &mut self,
    sink: &mut impl EventSink,
    cwd: &str,
  ) -> Result<()> {
    let mut active_session: Option<Uuid> = None;
    let mut prediction: Option<LocalBoxFuture<'_, Result<String>>> = None;
    let mut in_flight_queue_item: Option<Uuid> = None;

    loop {
      if self.shutdown.is_cancelled() {
        vlog!("Shutdown signal received, exiting harness loop...");
        break;
      }

      if prediction.is_some() && !sink.wants_prediction() {
        prediction = None;
      }

      let queued = match active_session {
        Some(session_id) => {
          self.sessions.claim_next_queued_message(session_id).await?
        }
        None => None,
      };

      let input = if let Some(message) = queued {
        in_flight_queue_item = Some(message.id);
        self.emit_queue(sink, active_session).await?;
        message.content
      } else {
        match sink.poll_input().await? {
          InputEvent::Pending => {
            if let Some(pending) = prediction.as_mut()
              && let Poll::Ready(result) = futures::poll!(pending)
            {
              prediction = None;
              match result {
                Ok(text) if !text.is_empty() => {
                  sink.on_event(AppEvent::Prediction { text }).await;
                }
                Ok(_) => {}
                Err(e) => {
                  vlog!("Failed to predict next prompt: {e}");
                }
              }
            }
            if prediction.is_some() {
              tokio::time::sleep(PREDICTION_YIELD).await;
            }
            continue;
          }
          InputEvent::Exit => break,
          InputEvent::Input(input) => input,
        }
      };

      prediction = None;

      let command = match Command::parse(&input) {
        Ok(Some(command)) => Some(command),
        Ok(None) => None,
        Err(e) => {
          self.complete_in_flight(&mut in_flight_queue_item).await?;
          sink
            .on_event(AppEvent::Error {
              message: e.to_string(),
            })
            .await;
          continue;
        }
      };

      if let Some(command) = command {
        let effect = match command.effect(active_session) {
          Ok(effect) => effect,
          Err(e) => {
            self.complete_in_flight(&mut in_flight_queue_item).await?;
            sink
              .on_event(AppEvent::Error {
                message: e.to_string(),
              })
              .await;
            continue;
          }
        };

        let cancel = self.shutdown.child_token();
        sink.set_cancel_token(cancel.clone()).await;

        let result = self
          .apply_effect(sink, active_session, effect, &cancel, cwd)
          .await;
        self.complete_in_flight(&mut in_flight_queue_item).await?;

        match result? {
          EffectOutcome::Continue => {}
          EffectOutcome::Exit => {
            sink.on_exit().await;
            break;
          }
          EffectOutcome::Activate(session_id) => {
            active_session = session_id;
          }
        }
        continue;
      }

      let session_id = match active_session {
        Some(session_id) => session_id,
        None => {
          let session_id = self.sessions.create_session(cwd).await?;
          vlog!("Created session with ID: {session_id}");
          active_session = Some(session_id);
          session_id
        }
      };

      let result = self
        .run_prompt(sink, session_id, PromptMode::Normal, Some(input))
        .await;
      self.complete_in_flight(&mut in_flight_queue_item).await?;
      let completed = result?;
      if completed
        && !self.sessions.has_queued_messages(session_id).await?
        && sink.wants_prediction()
        && self.sessions.predicts_next_prompt()
      {
        prediction =
          Some(Box::pin(self.sessions.predict_next_prompt(session_id)));
      }
    }
    return Ok(());
  }

  async fn complete_in_flight(
    &self,
    in_flight: &mut Option<Uuid>,
  ) -> Result<()> {
    if let Some(id) = in_flight.take() {
      self.sessions.complete_queued_message(id).await?;
    }
    return Ok(());
  }

  async fn emit_queue(
    &self,
    sink: &mut impl EventSink,
    session_id: Option<Uuid>,
  ) -> Result<()> {
    let messages = match session_id {
      Some(id) => self
        .sessions
        .queued_messages(id)
        .await?
        .into_iter()
        .map(|message| message.content)
        .collect(),
      None => Vec::new(),
    };
    sink.on_event(AppEvent::QueueUpdated { messages }).await;
    return Ok(());
  }

  async fn run_prompt(
    &mut self,
    sink: &mut impl EventSink,
    session_id: Uuid,
    mode: PromptMode,
    text: Option<String>,
  ) -> Result<bool> {
    sink
      .on_event(AppEvent::Processing { processing: true })
      .await;

    let cancel = self.shutdown.child_token();
    sink.set_cancel_token(cancel.clone()).await;
    sink
      .set_approval_channel(Some(self.sessions.approval_channel()))
      .await;

    let outcome =
      match run_turn_adapter(self, sink, session_id, mode, text, &cancel).await
      {
        Ok(outcome) => outcome,
        Err(e) => {
          sink
            .on_event(AppEvent::Error {
              message: e.to_string(),
            })
            .await;
          return Ok(false);
        }
      };

    if let Some(TurnOutcome::Completed { .. }) = outcome {
      let last_usage = self.sessions.last_usage(session_id);
      let context_window = match self.sessions.current_model(Some(session_id)) {
        Some(model) => {
          self.sessions.get_providers().context_window(&model).await
        }
        None => None,
      };
      sink
        .on_event(AppEvent::Usage {
          usage: last_usage,
          context_window,
        })
        .await;
      return Ok(true);
    }

    return Ok(false);
  }

  async fn apply_effect(
    &mut self,
    sink: &mut impl EventSink,
    active_session: Option<Uuid>,
    effect: Effect,
    cancel: &CancellationToken,
    cwd: &str,
  ) -> Result<EffectOutcome> {
    return match effect {
      Effect::CreateSession { persona, model } => {
        match persona {
          Some(name) => self.sessions.set_persona(None, &name).await?,
          None => self.sessions.reset_persona(),
        }
        if let Some(model) = model {
          self.sessions.set_model(None, model).await?;
        }
        let name = self.sessions.persona_name(None);
        let model = self.sessions.current_model(None);
        sink.on_new_session(name, model).await;
        self.emit_queue(sink, None).await?;
        Ok(EffectOutcome::Activate(None))
      }
      Effect::ListSessions => {
        let sessions = self.sessions.list_sessions().await?;
        let Some(session_id) =
          sink.on_sessions(sessions, active_session).await?
        else {
          return Ok(EffectOutcome::Continue);
        };
        self.sessions.load_session(session_id).await?;
        let session = self
          .sessions
          .session(session_id)
          .context("No active session")?;
        sink.on_session_selection(session).await;
        self.emit_queue(sink, Some(session_id)).await?;
        Ok(EffectOutcome::Activate(Some(session_id)))
      }
      Effect::ListPersonas => {
        let personas = PersonaManager::list();
        let current_persona_name = self.sessions.persona_name(active_session);
        if let Some(name) =
          sink.on_personas(personas, current_persona_name).await?
        {
          self.sessions.set_persona(active_session, &name).await?;
          sink.on_persona_selection(name).await;
        }
        Ok(EffectOutcome::Continue)
      }
      Effect::ListModels => {
        let models = self.sessions.get_providers().list_models();
        let current_model = self.sessions.current_model(active_session);
        if let Some(model) = sink.on_models(models, current_model).await? {
          self
            .sessions
            .set_model(active_session, model.clone())
            .await?;
          sink.on_model_selection(model).await;
        }
        Ok(EffectOutcome::Continue)
      }
      Effect::Compact { session_id } => {
        compact_session(self, sink, session_id, cancel).await;
        Ok(EffectOutcome::Continue)
      }
      Effect::StartReview { session_id } => {
        let session_id = match session_id {
          Some(session_id) => session_id,
          None => {
            let session_id = self.sessions.create_session(cwd).await?;
            vlog!("Created session with ID: {session_id}");
            session_id
          }
        };
        self
          .sessions
          .set_persona(Some(session_id), REVIEWER_PERSONA.name())
          .await?;
        sink
          .on_persona_selection(REVIEWER_PERSONA.name().to_string())
          .await;
        self
          .run_prompt(sink, session_id, PromptMode::Review, None)
          .await?;
        Ok(EffectOutcome::Activate(Some(session_id)))
      }
      Effect::ShowHelp => {
        sink
          .on_help(Command::available_commands(active_session.is_some()))
          .await;
        Ok(EffectOutcome::Continue)
      }
      Effect::Exit => Ok(EffectOutcome::Exit),
    };
  }
}

enum EffectOutcome {
  Continue,
  Exit,
  Activate(Option<Uuid>),
}

async fn run_turn_adapter(
  app: &mut App,
  sink: &mut impl EventSink,
  session_id: Uuid,
  mode: PromptMode,
  text: Option<String>,
  cancel: &CancellationToken,
) -> Result<Option<TurnOutcome>> {
  let (events, mut rx) = mpsc::channel::<SessionEvent>(256);
  let approvals = app.sessions.approval_channel();
  let context = app.sessions.context(session_id)?;
  let repository = context.repository.clone();
  let SessionContext {
    session,
    config,
    providers,
    repository: repo,
  } = context;

  let agent_context = AgentContext {
    config,
    providers,
    repo,
    events: &events,
    cancel,
  };
  let turn = run_turn(
    session,
    Uuid::now_v7(),
    Uuid::nil(),
    mode,
    text,
    &agent_context,
  );
  tokio::pin!(turn);

  let mut ticker = ui_ticker();
  let mut outcome = None;

  loop {
    tokio::select! {
      biased;
      _ = ticker.tick() => {
        persist_queued_input(sink, &repository, session_id).await;
      }
      event = rx.recv() => {
        if let Some(event) = event {
          handle_session_event(sink, &approvals, event).await;
        }
      }
      _ = cancel.cancelled() => {
        drain_cancelled_turn(&mut turn, &mut rx).await;
        break;
      }
      result = &mut turn => {
        outcome = Some(result);
        break;
      }
    }
  }

  if !cancel.is_cancelled() {
    while let Ok(event) = rx.try_recv() {
      handle_session_event(sink, &approvals, event).await;
    }
  }
  sink
    .on_event(AppEvent::Processing { processing: false })
    .await;

  return Ok(outcome);
}

async fn drain_cancelled_turn<F>(
  turn: &mut Pin<&mut F>,
  rx: &mut mpsc::Receiver<SessionEvent>,
) where
  F: Future<Output = TurnOutcome>,
{
  loop {
    tokio::select! {
      biased;
      _ = turn.as_mut() => break,
      _ = rx.recv() => {}
    }
  }
}

async fn compact_session(
  app: &mut App,
  sink: &mut impl EventSink,
  session_id: Uuid,
  cancel: &CancellationToken,
) {
  let (events, mut rx) = mpsc::channel::<SessionEvent>(256);
  let approvals = app.sessions.approval_channel();
  let context = match app.sessions.context(session_id) {
    Ok(context) => context,
    Err(e) => {
      vlog!("Failed to prepare compaction: {e}");
      return;
    }
  };
  let repository = context.repository.clone();
  let SessionContext {
    session,
    config,
    providers,
    repository: repo,
  } = context;

  let agent_context = AgentContext {
    config,
    providers,
    repo,
    events: &events,
    cancel,
  };
  let compact = run_compact(session, Uuid::now_v7(), 0, &agent_context);
  tokio::pin!(compact);

  let mut ticker = ui_ticker();
  let mut outcome = None;

  loop {
    tokio::select! {
      biased;
      _ = ticker.tick() => {
        persist_queued_input(sink, &repository, session_id).await;
      }
      event = rx.recv() => {
        if let Some(event) = event {
          handle_session_event(sink, &approvals, event).await;
        }
      }
      _ = cancel.cancelled() => {
        drain_cancelled_turn(&mut compact, &mut rx).await;
        break;
      }
      result = &mut compact => {
        outcome = Some(result);
        break;
      }
    }
  }

  if !cancel.is_cancelled() {
    while let Ok(event) = rx.try_recv() {
      handle_session_event(sink, &approvals, event).await;
    }
  }

  if cancel.is_cancelled() {
    sink
      .on_event(AppEvent::Processing { processing: false })
      .await;
    return;
  }

  if let Some(TurnOutcome::Failed { message }) = outcome {
    vlog!("Compaction failed: {message}");
  }
}

async fn handle_session_event(
  sink: &mut impl EventSink,
  approvals: &ApprovalChannel,
  event: SessionEvent,
) {
  match event {
    SessionEvent::Event(event) => {
      if let Some(event) = app_event(event) {
        sink.on_event(event).await;
      }
    }
    SessionEvent::ApprovalRequest { event, reply } => {
      let tool_call_id = match &event {
        ServerEvent::ToolApprovalRequest { tool_call_id, .. } => {
          tool_call_id.clone()
        }
        _ => {
          let _ = reply.send(ApprovalDecision::Denied);
          return;
        }
      };

      let mut decisions = approvals.subscribe();
      if let Some(event) = app_event(event) {
        sink.on_event(event).await;
      }

      let decision = match decisions.try_recv() {
        Ok(decision)
          if decision.tool_call_id == tool_call_id
            && decision.status == ToolApprovalStatus::Approved =>
        {
          ApprovalDecision::Approved
        }
        _ => ApprovalDecision::Denied,
      };
      let _ = reply.send(decision);
    }
    SessionEvent::Stopped { .. } => {}
  }
}

fn app_event(event: ServerEvent) -> Option<AppEvent> {
  return match event {
    ServerEvent::UserMessage { content, .. } => {
      Some(AppEvent::UserMessage { content })
    }
    ServerEvent::Token {
      text, reasoning, ..
    } => Some(AppEvent::Token { text, reasoning }),
    ServerEvent::ToolCall {
      id,
      name,
      arguments,
      preview,
      ..
    } => Some(AppEvent::ToolCall {
      id,
      name,
      arguments,
      preview,
    }),
    ServerEvent::ToolApprovalRequest {
      tool_call_id,
      tool_name,
      message,
      ..
    } => Some(AppEvent::ToolApprovalRequest {
      tool_call_id,
      tool_name,
      message,
    }),
    ServerEvent::ToolApprovalResult {
      tool_call_id,
      status,
      ..
    } => Some(AppEvent::ToolApprovalResult {
      tool_call_id,
      status,
    }),
    ServerEvent::ToolResult {
      id, result, error, ..
    } => Some(AppEvent::ToolResult { id, result, error }),
    ServerEvent::Usage {
      usage,
      context_window,
      ..
    } => Some(AppEvent::Usage {
      usage,
      context_window,
    }),
    ServerEvent::Subagents { delta, .. } => Some(AppEvent::Subagents { delta }),
    ServerEvent::Compacted { summary, .. } => {
      Some(AppEvent::Compacted { summary })
    }
    ServerEvent::Prediction { text, .. } => Some(AppEvent::Prediction { text }),
    ServerEvent::QueueUpdated { messages, .. } => {
      Some(AppEvent::QueueUpdated {
        messages: messages
          .into_iter()
          .map(|message| message.content)
          .collect(),
      })
    }
    ServerEvent::Error { message, .. } => Some(AppEvent::Error { message }),
    _ => None,
  };
}

fn ui_ticker() -> tokio::time::Interval {
  let mut ticker = interval(UI_TICK);
  ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
  return ticker;
}
