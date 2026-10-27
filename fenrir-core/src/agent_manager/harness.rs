use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::{Mutex, mpsc};

use crate::agent_manager::AgentManager;
use crate::app::App;
use crate::app::AppEvent;
use crate::app::EventSink;
use crate::app::command::{AppCommand, AppCommandResult};
use crate::vlog;

impl AgentManager {
  pub async fn harness_loop(
    app: &Arc<Mutex<App>>,
    sink: &mut impl EventSink,
  ) -> Result<String> {
    loop {
      {
        let app_locked = app.lock().await;
        if app_locked.shutdown.is_cancelled() {
          vlog!("Shutdown signal received, exiting harness loop...");
          break;
        }
      }

      let input = sink.read_input().await?;
      if let Some(input) = input {
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
          let cancel = {
            let app_locked = app.lock().await;
            app_locked.shutdown.child_token()
          };
          sink.set_cancel_token(cancel.clone());

          if matches!(&command, AppCommand::Compact) {
            let (result_tx, mut result_rx) = mpsc::channel(1);
            let app_clone = Arc::clone(app);
            let cancel_clone = cancel.clone();
            tokio::task::spawn(async move {
              let result = {
                let mut app_locked = app_clone.lock().await;
                app_locked
                  .agent_manager
                  .handle_command(command, &cancel_clone)
                  .await
              };
              result_tx.send(result).await.ok();
            });

            let mut command_done = false;
            let mut command_result = None;
            while !command_done {
              tokio::select! {
                biased;
                result = result_rx.recv() => {
                  command_result = result;
                  command_done = true;
                }
                _ = sink.poll_events() => {},
                _ = cancel.cancelled() => {
                  command_done = true;
                }
              }
            }

            if command_done && cancel.is_cancelled() {
              sink
                .on_event(AppEvent::Processing { processing: false })
                .await;
              continue;
            }

            match command_result {
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
            continue;
          }

          let result = {
            let mut app_locked = app.lock().await;
            app_locked
              .agent_manager
              .handle_command(command, &cancel)
              .await
          };
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
                let result = {
                  let mut app_locked = app.lock().await;
                  app_locked
                    .agent_manager
                    .handle_command(
                      AppCommand::LoadSession { id: session_id },
                      &cancel,
                    )
                    .await
                };
                if let AppCommandResult::Session(session) = result? {
                  sink.on_session_selection(session);
                }
              }
            }
            AppCommandResult::Session(session) => {
              sink.on_session_selection(session);
            }
            AppCommandResult::Personas { personas, current } => {
              if let Some(name) = sink.on_personas(personas, current)? {
                {
                  let mut app_locked = app.lock().await;
                  app_locked
                    .agent_manager
                    .handle_command(
                      AppCommand::SelectPersona { name: name.clone() },
                      &cancel,
                    )
                    .await?;
                }
                sink.on_persona_selection(name);
              }
            }
            AppCommandResult::Persona(name) => {
              sink.on_persona_selection(name);
            }
            AppCommandResult::Models { models, current } => {
              if let Some(model) = sink.on_models(models, current)? {
                let result = {
                  let mut app_locked = app.lock().await;
                  app_locked
                    .agent_manager
                    .handle_command(AppCommand::SelectModel { model }, &cancel)
                    .await
                };
                if let AppCommandResult::Model(model) = result? {
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

        {
          let mut app_locked = app.lock().await;
          app_locked
            .agent_manager
            .ensure_session_with_system_message()
            .await?;

          let session_id = app_locked
            .agent_manager
            .session_manager
            .get_current_session()
            .map(|s| s.id)
            .context("No active session")?;

          app_locked
            .agent_manager
            .session_manager
            .add_user_message_to_session(session_id, &input)
            .await?;
        }

        sink
          .on_event(AppEvent::UserMessage {
            content: input.clone(),
          })
          .await;
        sink
          .on_event(AppEvent::Processing { processing: true })
          .await;

        let cancel = {
          let app_locked = app.lock().await;
          app_locked.shutdown.child_token()
        };
        sink.set_cancel_token(cancel.clone());

        let (tx, mut rx) = mpsc::channel::<AppEvent>(256);
        let app_clone = Arc::clone(app);
        let cancel_clone = cancel.clone();

        let chat_handle = tokio::task::spawn(async move {
          let mut app_locked = app_clone.lock().await;
          let result = app_locked
            .agent_manager
            .process_chat(&tx, &cancel_clone)
            .await;
          drop(app_locked);
          tx.send(AppEvent::Processing { processing: false })
            .await
            .ok();
          result
        });

        let mut chat_done = false;
        let mut chat_result = None;

        while !chat_done {
          tokio::select! {
            biased;
            event = rx.recv() => match event {
              Some(e) => {
                sink.on_event(e).await;
              }
              None => {
                chat_done = true;
              }
            },
            _ = sink.poll_events() => {}
            _ = cancel.cancelled() => {
              sink.on_event(AppEvent::Processing { processing: false }).await;
              while let Some(event) = rx.recv().await {
                if let AppEvent::ToolConfirmation { reply, .. } = event {
                  reply.send(false).ok();
                }
              }
              continue;
            }
          }
        }

        if !cancel.is_cancelled() {
          chat_result = Some(chat_handle.await);
        }

        if let Some(result) = chat_result {
          match result {
            Ok(Ok(_)) => {
              let last_usage = {
                let app_locked = app.lock().await;
                app_locked.agent_manager.last_usage()
              };
              sink.on_event(AppEvent::Usage { usage: last_usage }).await;
              let (compact_tx, mut compact_rx) = mpsc::channel(1);
              let app_compact_clone = app.clone();
              let cancel_clone = cancel.clone();
              tokio::task::spawn(async move {
                let result =
                  Self::maybe_compact_arc(app_compact_clone, &cancel_clone)
                    .await;
                compact_tx.send(result).await.ok();
              });

              let mut compact_done = false;
              while !compact_done {
                tokio::select! {
                  biased;
                  result = compact_rx.recv() => {
                    compact_done = true;
                    match result {
                      Some(Ok(summary)) if !summary.is_empty() => {
                        sink.on_event(AppEvent::Compacted { summary }).await;
                      }
                      Some(Err(e))
                        if !cancel.is_cancelled() => {
                          sink.on_event(AppEvent::Error {
                            message: e.to_string(),
                          })
                          .await;
                        }
                      _ => {}
                    }
                  }
                  _ = sink.poll_events() => {},
                  _ = cancel.cancelled() => {
                    compact_done = true;
                  }
                }
              }
            }
            Ok(Err(e)) => {
              sink
                .on_event(AppEvent::Error {
                  message: e.to_string(),
                })
                .await;
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
      } else {
        break;
      }
    }
    return Ok(String::new());
  }
}
