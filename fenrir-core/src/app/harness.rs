use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::app::App;
use crate::app::AppEvent;
use crate::app::ApprovalChannel;
use crate::app::EventSink;
use crate::app::command::{AppCommand, AppCommandResult};
use crate::persona_manager::PersonaManager;
use crate::slash_commands::SlashCommand;
use crate::vlog;

const UI_TICK: Duration = Duration::from_millis(16);

impl App {
  fn approval_channel(
    &self,
    session_id: Option<Uuid>,
  ) -> Option<ApprovalChannel> {
    return session_id
      .and_then(|id| self.sessions.session(id))
      .map(|session| session.approval_channel().clone());
  }

  pub async fn harness_loop(
    &mut self,
    sink: &mut impl EventSink,
  ) -> Result<String> {
    let mut current_session_id: Option<Uuid> = None;
    loop {
      if self.shutdown.is_cancelled() {
        vlog!("Shutdown signal received, exiting harness loop...");
        break;
      }

      let Some(input) = sink.read_input().await? else {
        break;
      };

      let command = match AppCommand::parse(&input) {
        Ok(Some(command)) => Some(command),
        Ok(None) => None,
        Err(e) => {
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

        if matches!(&command, AppCommand::Compact) {
          match current_session_id {
            Some(session_id) => {
              run_compact(self, sink, session_id, &cancel).await;
            }
            None => {
              sink
                .on_event(AppEvent::Error {
                  message: "No session selected".to_string(),
                })
                .await;
            }
          }
          continue;
        }

        let result = self
          .handle_command(command, &mut current_session_id, &cancel)
          .await?;
        match result {
          AppCommandResult::Exit => {
            sink.on_exit().await;
            break;
          }
          AppCommandResult::Help(slash_commands) => {
            sink.on_help(slash_commands).await;
          }
          AppCommandResult::Sessions { sessions } => {
            let selected =
              sink.on_sessions(sessions, current_session_id).await?;
            if let Some(session_id) = selected {
              self.sessions.load_session(session_id).await?;
              current_session_id = Some(session_id);
              let session = self
                .sessions
                .session(session_id)
                .context("No active session")?;
              sink.on_session_selection(session).await;
            }
          }
          AppCommandResult::Session(session_id) => {
            let session = self
              .sessions
              .session(session_id)
              .context("No active session")?;
            sink.on_session_selection(session).await;
          }
          AppCommandResult::Personas {
            personas,
            current_persona_name,
          } => {
            if let Some(name) =
              sink.on_personas(personas, current_persona_name).await?
            {
              self.sessions.set_persona(current_session_id, &name)?;
              sink.on_persona_selection(name).await;
            }
          }
          AppCommandResult::Persona(name) => {
            sink.on_persona_selection(name).await;
          }
          AppCommandResult::Models {
            models,
            current_model,
          } => {
            if let Some(model) = sink.on_models(models, current_model).await? {
              self.sessions.set_model(current_session_id, model.clone());
              sink.on_model_selection(model).await;
            }
          }
          AppCommandResult::Model(model) => {
            sink.on_model_selection(model).await;
          }
          AppCommandResult::Compacted { summary } => {
            sink.on_event(AppEvent::Compacted { summary }).await;
          }
          AppCommandResult::Noop => {}
        }
        sink
          .set_approval_channel(self.approval_channel(current_session_id))
          .await;
        continue;
      }

      let session_id = match current_session_id {
        Some(session_id) => session_id,
        None => {
          let session_id = self.sessions.create_session().await?;
          vlog!("Created session with ID: {session_id}");
          current_session_id = Some(session_id);
          session_id
        }
      };

      self.sessions.ensure_system_message(session_id).await?;
      self.sessions.add_user_message(session_id, &input).await?;
      sink
        .set_approval_channel(self.approval_channel(Some(session_id)))
        .await;

      sink
        .on_event(AppEvent::UserMessage {
          content: input.clone(),
        })
        .await;
      sink
        .on_event(AppEvent::Processing { processing: true })
        .await;

      let cancel = self.shutdown.child_token();
      sink.set_cancel_token(cancel.clone()).await;

      let Some(result) = run_chat(self, sink, session_id, &cancel).await else {
        continue;
      };

      match result {
        Ok(_) => {
          let last_usage = self.sessions.last_usage(session_id);
          let context_window =
            match self.sessions.current_model(Some(session_id)) {
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
          run_compaction(self, sink, session_id, &cancel).await;
        }
        Err(e) => {
          sink
            .on_event(AppEvent::Error {
              message: e.to_string(),
            })
            .await;
        }
      }
    }
    return Ok(String::new());
  }

  async fn handle_command(
    &mut self,
    command: AppCommand,
    current_session_id: &mut Option<Uuid>,
    cancel: &CancellationToken,
  ) -> Result<AppCommandResult> {
    let has_session = current_session_id.is_some();
    return match command {
      AppCommand::Exit => Ok(AppCommandResult::Exit),
      AppCommand::Help => Ok(AppCommandResult::Help(
        SlashCommand::available_commands(has_session),
      )),
      AppCommand::ListSessions => {
        let sessions = self.sessions.list_sessions().await?;
        return Ok(AppCommandResult::Sessions { sessions });
      }
      AppCommand::LoadSession { id } => {
        self.sessions.load_session(id).await?;
        *current_session_id = Some(id);
        return Ok(AppCommandResult::Session(id));
      }
      AppCommand::ListPersonas => {
        let personas = PersonaManager::list();
        let current_persona_name =
          self.sessions.persona_name(*current_session_id);
        return Ok(AppCommandResult::Personas {
          personas,
          current_persona_name,
        });
      }
      AppCommand::SelectPersona { name } => {
        self.sessions.set_persona(*current_session_id, &name)?;
        return Ok(AppCommandResult::Noop);
      }
      AppCommand::ListModels => {
        let models = self.sessions.get_providers().list_models();
        let current_model = self.sessions.current_model(*current_session_id);
        return Ok(AppCommandResult::Models {
          models,
          current_model,
        });
      }
      AppCommand::SelectModel { model } => {
        self.sessions.set_model(*current_session_id, model.clone());
        return Ok(AppCommandResult::Model(model));
      }
      AppCommand::Compact => {
        let id = current_session_id.context("No session selected")?;
        let summary = self.sessions.compact(id, cancel).await?;
        return Ok(AppCommandResult::Compacted { summary });
      }
      AppCommand::New => {
        *current_session_id = None;
        self.sessions.reset_persona();
        let name = self.sessions.persona_name(None);
        return Ok(AppCommandResult::Persona(name));
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
  let chat = app.sessions.run_chat(session_id, &tx, cancel);
  tokio::pin!(chat);

  let mut ticker = ui_ticker();
  let mut outcome = None;

  loop {
    tokio::select! {
      biased;
      _ = ticker.tick() => sink.poll_events().await,
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
  cancel: &CancellationToken,
) {
  let compact = app.sessions.maybe_compact(session_id, cancel);
  tokio::pin!(compact);

  let mut ticker = ui_ticker();

  loop {
    tokio::select! {
      biased;
      _ = ticker.tick() => sink.poll_events().await,
      result = &mut compact => {
        match result {
          Ok(summary) if !summary.is_empty() => {
            sink.on_event(AppEvent::Compacted { summary }).await;
          }
          Err(e) if !cancel.is_cancelled() => {
            sink
              .on_event(AppEvent::Error {
                message: e.to_string(),
              })
              .await;
          }
          _ => {}
        }
        break;
      }
      _ = cancel.cancelled() => {
        let _ = compact.as_mut().await;
        break;
      }
    }
  }
}

async fn run_compact(
  app: &mut App,
  sink: &mut impl EventSink,
  session_id: Uuid,
  cancel: &CancellationToken,
) {
  let command = app.sessions.compact(session_id, cancel);
  tokio::pin!(command);

  let mut ticker = ui_ticker();
  let mut result = None;

  loop {
    tokio::select! {
      biased;
      _ = ticker.tick() => sink.poll_events().await,
      r = &mut command => {
        result = Some(r);
        break;
      }
      _ = cancel.cancelled() => {
        let _ = command.as_mut().await;
        break;
      }
    }
  }

  if cancel.is_cancelled() {
    sink
      .on_event(AppEvent::Processing { processing: false })
      .await;
    return;
  }

  match result {
    Some(Ok(summary)) => {
      sink.on_event(AppEvent::Compacted { summary }).await;
    }
    Some(Err(e)) => {
      sink
        .on_event(AppEvent::Error {
          message: e.to_string(),
        })
        .await;
    }
    None => {}
  }
}

fn ui_ticker() -> tokio::time::Interval {
  let mut ticker = interval(UI_TICK);
  ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
  return ticker;
}
