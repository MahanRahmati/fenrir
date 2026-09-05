use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;

use crate::app::App;
use crate::app::AppEvent;
use crate::app::EventSink;
use crate::app::command::{AppCommand, AppCommandResult};
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::vlog;

impl App {
  pub async fn harness_loop(
    app: &Arc<Mutex<App>>,
    sink: &mut impl EventSink,
  ) -> RuntimeResult<String> {
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
            sink.on_event(AppEvent::Error {
              message: e.to_string(),
            });
            continue;
          }
        };

        if let Some(command) = command {
          let cancel = CancellationToken::new();
          sink.set_cancel_token(cancel.clone());

          if matches!(&command, AppCommand::Compact) {
            let (result_tx, mut result_rx) = mpsc::channel(1);
            let app_clone = Arc::clone(app);
            let cancel_clone = cancel.clone();
            tokio::task::spawn(async move {
              let result = {
                let mut app_locked = app_clone.lock().await;
                app_locked.handle_command(command, &cancel_clone).await
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
              sink.on_event(AppEvent::Processing { processing: false });
              continue;
            }

            match command_result {
              Some(Ok(AppCommandResult::Compacted { summary })) => {
                sink.on_event(AppEvent::Compacted { summary });
              }
              Some(Ok(_)) => {}
              Some(Err(e)) => {
                sink.on_event(AppEvent::Error {
                  message: e.to_string(),
                });
              }
              None => {}
            }
            continue;
          }

          let result = {
            let mut app_locked = app.lock().await;
            app_locked.handle_command(command, &cancel).await
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
            AppCommandResult::Compacted { summary } => {
              sink.on_event(AppEvent::Compacted { summary });
            }
            AppCommandResult::Noop => {}
          }
          continue;
        }

        {
          let mut app_locked = app.lock().await;
          app_locked.ensure_session_with_system_message().await?;

          app_locked
            .session_manager
            .add_user_message_to_current_session(&input)
            .await
            .map_err(|e| RuntimeError::Session(e.to_string()))?;
        }

        sink.on_event(AppEvent::UserMessage {
          content: input.clone(),
        });
        sink.on_event(AppEvent::Processing { processing: true });

        let cancel = CancellationToken::new();
        sink.set_cancel_token(cancel.clone());

        let (tx, mut rx) = mpsc::channel::<AppEvent>(256);
        let app_clone = Arc::clone(app);
        let cancel_clone = cancel.clone();

        let mut chat_handle = tokio::task::spawn(async move {
          let mut app_locked = app_clone.lock().await;
          let result = app_locked.process_chat(&tx, &cancel_clone).await;
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
              Some(AppEvent::ToolConfirmation { message, reply }) => {
                let confirmed = sink.confirm(&message);
                reply.send(confirmed).ok();
              }
              Some(e) => {
                sink.on_event(e);
              }
              None => {
                chat_done = true;
              }
            },
            result = &mut chat_handle => {
              chat_result = Some(result);
              chat_done = true;
            }
            _ = sink.poll_events() => {}
            _ = cancel.cancelled() => {
              sink.on_event(AppEvent::Processing { processing: false });
              while let Some(event) = rx.recv().await {
                if let AppEvent::ToolConfirmation { reply, .. } = event {
                  reply.send(false).ok();
                }
              }
              continue;
            }
          }
        }

        if let Some(result) = chat_result {
          match result {
            Ok(Ok(_)) => {
              let last_usage = {
                let app_locked = app.lock().await;
                app_locked.last_usage()
              };
              sink.on_event(AppEvent::Usage { usage: last_usage });
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
                        sink.on_event(AppEvent::Compacted { summary });
                      }
                      Some(Err(e))
                        if !cancel.is_cancelled() => {
                          sink.on_event(AppEvent::Error {
                            message: e.to_string(),
                          });
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
              sink.on_event(AppEvent::Error {
                message: e.to_string(),
              });
            }
            Err(e) => {
              sink.on_event(AppEvent::Error {
                message: e.to_string(),
              });
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
