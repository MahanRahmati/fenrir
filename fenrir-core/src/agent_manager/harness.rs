use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};
use tokio_util::sync::CancellationToken;

use crate::agent_manager::AgentManager;
use crate::app::App;
use crate::app::AppEvent;
use crate::app::EventSink;
use crate::app::command::{AppCommand, AppCommandResult};
use crate::vlog;

const UI_TICK: Duration = Duration::from_millis(16);

impl AgentManager {
  pub async fn harness_loop(
    app: &mut App,
    sink: &mut impl EventSink,
  ) -> Result<String> {
    loop {
      if app.shutdown.is_cancelled() {
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
        let cancel = app.shutdown.child_token();
        sink.set_cancel_token(cancel.clone());

        if matches!(&command, AppCommand::Compact) {
          run_compact(app, sink, &cancel).await;
          continue;
        }

        let result = app.agent_manager.handle_command(command, &cancel).await;
        match result? {
          AppCommandResult::Exit => {
            sink.on_exit();
            break;
          }
          AppCommandResult::Help(slash_commands) => {
            sink.on_help(slash_commands);
          }
          AppCommandResult::Sessions { sessions, current } => {
            if let Some(session_id) = sink.on_sessions(sessions, current)? {
              let result = app
                .agent_manager
                .handle_command(
                  AppCommand::LoadSession { id: session_id },
                  &cancel,
                )
                .await?;
              if let AppCommandResult::Session(session) = result {
                sink.on_session_selection(session);
              }
            }
          }
          AppCommandResult::Session(session) => {
            sink.on_session_selection(session);
          }
          AppCommandResult::Personas { personas, current } => {
            if let Some(name) = sink.on_personas(personas, current)? {
              app
                .agent_manager
                .handle_command(
                  AppCommand::SelectPersona { name: name.clone() },
                  &cancel,
                )
                .await?;
              sink.on_persona_selection(name);
            }
          }
          AppCommandResult::Persona(name) => {
            sink.on_persona_selection(name);
          }
          AppCommandResult::Models { models, current } => {
            if let Some(model) = sink.on_models(models, current)? {
              let result = app
                .agent_manager
                .handle_command(AppCommand::SelectModel { model }, &cancel)
                .await?;
              if let AppCommandResult::Model(model) = result {
                sink.on_model_selection(model);
              }
            }
          }
          AppCommandResult::Model(model) => {
            sink.on_model_selection(model);
          }
          AppCommandResult::Compacted { summary } => {
            sink.on_event(AppEvent::Compacted { summary }).await;
          }
          AppCommandResult::Noop => {}
        }
        continue;
      }

      app
        .agent_manager
        .ensure_session_with_system_message()
        .await?;

      let session_id = app
        .agent_manager
        .session_manager
        .get_current_session()
        .map(|s| s.id)
        .context("No active session")?;

      app
        .agent_manager
        .session_manager
        .add_user_message_to_session(session_id, &input)
        .await?;

      sink
        .on_event(AppEvent::UserMessage {
          content: input.clone(),
        })
        .await;
      sink
        .on_event(AppEvent::Processing { processing: true })
        .await;

      let cancel = app.shutdown.child_token();
      sink.set_cancel_token(cancel.clone());

      let Some(result) = run_chat(app, sink, &cancel).await else {
        continue;
      };

      match result {
        Ok(_) => {
          let last_usage = app.agent_manager.last_usage();
          sink.on_event(AppEvent::Usage { usage: last_usage }).await;
          run_compaction(app, sink, &cancel).await;
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
}

async fn run_chat(
  app: &mut App,
  sink: &mut impl EventSink,
  cancel: &CancellationToken,
) -> Option<Result<String>> {
  let (tx, mut rx) = mpsc::channel::<AppEvent>(256);
  let chat = app.agent_manager.process_chat(&tx, cancel);
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
  cancel: &CancellationToken,
) {
  let compact = app.agent_manager.maybe_compact(cancel);
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
  cancel: &CancellationToken,
) {
  let command = app
    .agent_manager
    .handle_command(AppCommand::Compact, cancel);
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
    Some(Ok(AppCommandResult::Compacted { summary })) => {
      sink.on_event(AppEvent::Compacted { summary }).await;
    }
    Some(Ok(_)) => {}
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
