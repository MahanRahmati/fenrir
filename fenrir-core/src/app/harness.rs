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

use crate::app::App;
use crate::app::AppEvent;
use crate::app::ApprovalChannel;
use crate::app::EventSink;
use crate::app::InputEvent;
use crate::app::command::{AppCommand, CommandOutcome};
use crate::persona_manager::{PersonaManager, REVIEWER_PERSONA};
use crate::session_manager::CompactionMode;
use crate::session_manager::SessionContext;
use crate::session_manager::SessionRepository;
use crate::slash_commands::SlashCommand;
use crate::vlog;

const UI_TICK: Duration = Duration::from_millis(16);
const PREDICTION_YIELD: Duration = Duration::from_millis(1);
const CWD_REVIEW_PROMPT: &str = "Review the uncommitted changes in the current working directory. Use the git tool with `status` to list changed files and `diff` to inspect unstaged changes, and read the changed files you need. Report bugs, risky behavior changes, and clear deviations from existing patterns. Do not modify any files.";
const SESSION_REVIEW_PROMPT: &str = "Review the changes made in this session. Use the conversation to identify what changed, inspect the current state of those files, and use the git tool with `status` to list changed files and `diff` to inspect unstaged changes. Report bugs, risky behavior changes, and clear deviations from existing patterns. Do not modify any files.";

async fn persist_queued_input(
  sink: &mut impl EventSink,
  repository: &SessionRepository,
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
  fn approval_channel(
    &self,
    session_id: Option<Uuid>,
  ) -> Option<ApprovalChannel> {
    return session_id
      .and_then(|id| self.sessions.session(id))
      .map(|session| session.approval_channel().clone());
  }

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

      let command = match AppCommand::parse(&input) {
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
        let cancel = self.shutdown.child_token();
        sink.set_cancel_token(cancel.clone()).await;

        let result = self
          .command(sink, active_session, command, &cancel, cwd)
          .await;
        self.complete_in_flight(&mut in_flight_queue_item).await?;

        match result? {
          CommandOutcome::Continue => {}
          CommandOutcome::Exit => {
            sink.on_exit().await;
            break;
          }
          CommandOutcome::Activate(session_id) => {
            active_session = session_id;
          }
        }
        sink
          .set_approval_channel(self.approval_channel(active_session))
          .await;
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

      let result = self.prompt(sink, session_id, &input).await;
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

  pub async fn prompt(
    &mut self,
    sink: &mut impl EventSink,
    session_id: Uuid,
    input: &str,
  ) -> Result<bool> {
    self.sessions.ensure_system_message(session_id).await?;
    self.sessions.add_user_message(session_id, input).await?;
    sink
      .set_approval_channel(self.approval_channel(Some(session_id)))
      .await;

    sink
      .on_event(AppEvent::UserMessage {
        content: input.to_string(),
      })
      .await;
    sink
      .on_event(AppEvent::Processing { processing: true })
      .await;

    let cancel = self.shutdown.child_token();
    sink.set_cancel_token(cancel.clone()).await;

    let Some(result) = run_chat(self, sink, session_id, &cancel).await else {
      return Ok(false);
    };

    match result {
      Ok(_) => {
        let last_usage = self.sessions.last_usage(session_id);
        let context_window = match self.sessions.current_model(Some(session_id))
        {
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
        run_compaction(self, sink, session_id, CompactionMode::Auto, &cancel)
          .await;
        return Ok(true);
      }
      Err(e) => {
        sink
          .on_event(AppEvent::Error {
            message: e.to_string(),
          })
          .await;
        return Ok(false);
      }
    }
  }

  pub async fn command(
    &mut self,
    sink: &mut impl EventSink,
    active_session: Option<Uuid>,
    command: AppCommand,
    cancel: &CancellationToken,
    cwd: &str,
  ) -> Result<CommandOutcome> {
    return match command {
      AppCommand::Exit => Ok(CommandOutcome::Exit),
      AppCommand::Help => {
        sink
          .on_help(SlashCommand::available_commands(active_session.is_some()))
          .await;
        Ok(CommandOutcome::Continue)
      }
      AppCommand::ListSessions => {
        let sessions = self.sessions.list_sessions().await?;
        let Some(session_id) =
          sink.on_sessions(sessions, active_session).await?
        else {
          return Ok(CommandOutcome::Continue);
        };
        self.sessions.load_session(session_id).await?;
        let session = self
          .sessions
          .session(session_id)
          .context("No active session")?;
        sink.on_session_selection(session).await;
        self.emit_queue(sink, Some(session_id)).await?;
        Ok(CommandOutcome::Activate(Some(session_id)))
      }
      AppCommand::ListPersonas => {
        let personas = PersonaManager::list();
        let current_persona_name = self.sessions.persona_name(active_session);
        if let Some(name) =
          sink.on_personas(personas, current_persona_name).await?
        {
          self.sessions.set_persona(active_session, &name)?;
          sink.on_persona_selection(name).await;
        }
        Ok(CommandOutcome::Continue)
      }
      AppCommand::ListModels => {
        let models = self.sessions.get_providers().list_models();
        let current_model = self.sessions.current_model(active_session);
        if let Some(model) = sink.on_models(models, current_model).await? {
          self.sessions.set_model(active_session, model.clone());
          sink.on_model_selection(model).await;
        }
        Ok(CommandOutcome::Continue)
      }
      AppCommand::Compact => {
        let Some(session_id) = active_session else {
          sink
            .on_event(AppEvent::Error {
              message: "No session selected".to_string(),
            })
            .await;
          return Ok(CommandOutcome::Continue);
        };
        run_compaction(self, sink, session_id, CompactionMode::Forced, cancel)
          .await;
        Ok(CommandOutcome::Continue)
      }
      AppCommand::Review => {
        let session_id = match active_session {
          Some(session_id) => session_id,
          None => {
            let session_id = self.sessions.create_session(cwd).await?;
            vlog!("Created session with ID: {session_id}");
            session_id
          }
        };
        let is_empty = self
          .sessions
          .session(session_id)
          .map(|session| session.messages.is_empty())
          .unwrap_or(true);
        self
          .sessions
          .set_persona(Some(session_id), REVIEWER_PERSONA.name())?;
        sink
          .on_persona_selection(REVIEWER_PERSONA.name().to_string())
          .await;
        let prompt = if is_empty {
          CWD_REVIEW_PROMPT
        } else {
          SESSION_REVIEW_PROMPT
        };
        self.prompt(sink, session_id, prompt).await?;
        Ok(CommandOutcome::Activate(Some(session_id)))
      }
      AppCommand::New => {
        self.sessions.reset_persona();
        let name = self.sessions.persona_name(None);
        let model = self.sessions.current_model(None);
        sink.on_new_session(name, model).await;
        self.emit_queue(sink, None).await?;
        Ok(CommandOutcome::Activate(None))
      }
    };
  }
}

async fn run_chat(
  app: &mut App,
  sink: &mut impl EventSink,
  session_id: Uuid,
  cancel: &CancellationToken,
) -> Option<Result<String>> {
  let (tx, mut rx) = mpsc::channel::<AppEvent>(256);

  let context = match app.sessions.context(session_id) {
    Ok(context) => context,
    Err(e) => {
      return Some(Err(e));
    }
  };
  let SessionContext {
    session,
    config,
    providers,
    repository,
  } = context;

  let chat = session.harness_loop(config, providers, repository, &tx, cancel);
  tokio::pin!(chat);

  let mut ticker = ui_ticker();
  let mut outcome = None;

  loop {
    tokio::select! {
      biased;
      _ = ticker.tick() => {
        persist_queued_input(sink, repository, session_id).await;
      }
      event = rx.recv() => {
        if let Some(event) = event {
          sink.on_event(event).await;
        }
      }
      _ = cancel.cancelled() => {
        drain_cancelled_turn(&mut chat, &mut rx).await;
        break;
      }
      result = &mut chat => {
        outcome = Some(result);
        break;
      }
    }
  }

  if !cancel.is_cancelled() {
    while let Ok(event) = rx.try_recv() {
      sink.on_event(event).await;
    }
  }
  sink
    .on_event(AppEvent::Processing { processing: false })
    .await;

  if cancel.is_cancelled() {
    return None;
  }
  return outcome;
}

async fn drain_cancelled_turn<F>(
  chat: &mut Pin<&mut F>,
  rx: &mut mpsc::Receiver<AppEvent>,
) where
  F: Future<Output = Result<String>>,
{
  loop {
    tokio::select! {
      biased;
      _ = chat.as_mut() => break,
      _ = rx.recv() => {}
    }
  }
}

async fn run_compaction(
  app: &mut App,
  sink: &mut impl EventSink,
  session_id: Uuid,
  mode: CompactionMode,
  cancel: &CancellationToken,
) {
  let context = match app.sessions.context(session_id) {
    Ok(context) => context,
    Err(e) => {
      vlog!("Failed to prepare compaction: {e}");
      return;
    }
  };
  let SessionContext {
    session,
    config,
    providers,
    repository,
  } = context;

  let compact = session.compact(config, providers, repository, mode, cancel);
  tokio::pin!(compact);

  let mut ticker = ui_ticker();
  let mut result = None;

  loop {
    tokio::select! {
      biased;
      _ = ticker.tick() => {
        persist_queued_input(sink, repository, session_id).await;
      }
      r = &mut compact => {
        result = Some(r);
        break;
      }
      _ = cancel.cancelled() => {
        let _ = compact.as_mut().await;
        break;
      }
    }
  }

  if mode == CompactionMode::Forced && cancel.is_cancelled() {
    sink
      .on_event(AppEvent::Processing { processing: false })
      .await;
    return;
  }

  match result {
    Some(Ok(summary))
      if mode == CompactionMode::Forced || !summary.is_empty() =>
    {
      sink.on_event(AppEvent::Compacted { summary }).await;
    }
    Some(Err(e)) if !cancel.is_cancelled() => {
      sink
        .on_event(AppEvent::Error {
          message: e.to_string(),
        })
        .await;
    }
    _ => {}
  }
}

fn ui_ticker() -> tokio::time::Interval {
  let mut ticker = interval(UI_TICK);
  ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
  return ticker;
}
