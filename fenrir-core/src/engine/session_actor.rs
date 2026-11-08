use std::future::Future;
use std::pin::Pin;

use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::agent::{AgentContext, run_compact, run_turn};
use crate::config::Config;
use crate::engine::protocol::{
  Correlation, PromptMode, ServerEvent, SessionEvent, TurnOutcome,
};
use crate::engine::repo_actor::RepoHandle;
use crate::persona_manager::PersonaManager;
use crate::provider_manager::{Model, ProviderManager};
use crate::session_manager::session::Session;

pub struct SessionHandle {
  pub requests: Sender<SessionRequest>,
  pub control: Sender<Control>,
}

pub enum SessionRequest {
  Turn {
    request_id: Option<u64>,
    client_id: Uuid,
    mode: PromptMode,
    text: Option<String>,
  },
  Compact {
    request_id: u64,
    client_id: Uuid,
  },
  SetPersona {
    request_id: u64,
    name: String,
  },
  SetModel {
    request_id: u64,
    model: Model,
  },
  Predict,
  Shutdown {
    reply: Option<oneshot::Sender<()>>,
  },
}

pub enum Control {
  Cancel { turn_id: Option<Uuid> },
}

type TurnFuture = Pin<Box<dyn Future<Output = TurnResult> + Send>>;

struct TurnStart {
  request_id: Option<u64>,
  client_id: Uuid,
  mode: PromptMode,
  text: Option<String>,
}

struct TurnResult {
  session: Session,
  turn_id: Uuid,
  outcome: TurnOutcome,
}

pub struct SessionActor {
  id: Uuid,
  session: Option<Session>,
  config: Config,
  repo: RepoHandle,
  events: Sender<SessionEvent>,
  inbox: Receiver<SessionRequest>,
  control: Receiver<Control>,
  current_queued: Option<Uuid>,
}

impl SessionActor {
  pub fn spawn(
    session: Session,
    config: Config,
    repo: RepoHandle,
    events: Sender<SessionEvent>,
  ) -> SessionHandle {
    let id = session.id;
    let (request_tx, request_rx) = mpsc::channel(64);
    let (control_tx, control_rx) = mpsc::channel(64);
    let actor = SessionActor {
      id,
      session: Some(session),
      config,
      repo,
      events,
      inbox: request_rx,
      control: control_rx,
      current_queued: None,
    };
    tokio::spawn(actor.run());
    return SessionHandle {
      requests: request_tx,
      control: control_tx,
    };
  }

  async fn run(mut self) {
    let mut turn: Option<TurnFuture> = None;
    let mut current_turn: Option<Uuid> = None;
    let mut current_cancel: Option<CancellationToken> = None;
    let mut shutdown_reply: Option<oneshot::Sender<()>> = None;
    let mut shutting_down = false;

    if let Err(e) = self.repo.reset_processing_queued_messages(self.id).await {
      self
        .emit(ServerEvent::Error {
          request_id: None,
          session_id: Some(self.id),
          message: format!("Failed to reset queued messages: {e}"),
        })
        .await;
    }
    self
      .start_next_queued(&mut turn, &mut current_turn, &mut current_cancel)
      .await;

    loop {
      tokio::select! {
        request = self.inbox.recv() => {
          let Some(request) = request else {
            if let Some(cancel) = &current_cancel {
              cancel.cancel();
            }
            break;
          };
          match request {
            SessionRequest::Turn {
              request_id,
              client_id,
              mode,
              text,
            } => {
              if shutting_down {
                continue;
              }
              if turn.is_some() {
                match mode {
                  PromptMode::Normal => self.enqueue_prompt(text).await,
                  PromptMode::Review => {
                    self.emit(ServerEvent::Error {
                      request_id,
                      session_id: Some(self.id),
                      message: "Session is busy; retry review when the turn finishes."
                        .to_string(),
                    }).await;
                  }
                }
              } else {
                self.start_turn(
                  TurnStart {
                    request_id,
                    client_id,
                    mode,
                    text,
                  },
                  &mut turn,
                  &mut current_turn,
                  &mut current_cancel,
                ).await;
              }
            }
            SessionRequest::Compact {
              request_id,
              client_id,
            } => {
              if shutting_down {
                continue;
              }
              if turn.is_some() {
                self.emit(ServerEvent::Error {
                  request_id: Some(request_id),
                  session_id: Some(self.id),
                  message: "Session is busy; retry compaction when the turn finishes."
                    .to_string(),
                }).await;
              } else {
                self.start_compact(
                  request_id,
                  client_id,
                  &mut turn,
                  &mut current_turn,
                  &mut current_cancel,
                ).await;
              }
            }
            SessionRequest::SetPersona { request_id, name } => {
              if shutting_down {
                continue;
              }
              if turn.is_some() {
                self.emit(ServerEvent::Error {
                  request_id: Some(request_id),
                  session_id: Some(self.id),
                  message: "Session is busy; retry persona change when the turn finishes."
                    .to_string(),
                }).await;
              } else {
                self.set_persona(request_id, &name).await;
              }
            }
            SessionRequest::SetModel { request_id, model } => {
              if shutting_down {
                continue;
              }
              if turn.is_some() {
                self.emit(ServerEvent::Error {
                  request_id: Some(request_id),
                  session_id: Some(self.id),
                  message: "Session is busy; retry model change when the turn finishes."
                    .to_string(),
                }).await;
              } else {
                self.set_model(request_id, model).await;
              }
            }
            SessionRequest::Predict => {
              if shutting_down {
                continue;
              }
              if turn.is_some() {
                self
                  .emit(ServerEvent::Prediction {
                    session_id: self.id,
                    text: String::new(),
                  })
                  .await;
                continue;
              }
              let Some(session) = self.session.as_ref() else {
                continue;
              };
              crate::prompt::prediction::spawn(
                session,
                &self.config,
                self.events.clone(),
              );
            }
            SessionRequest::Shutdown { reply } => {
              shutting_down = true;
              shutdown_reply = reply;
              if let Some(cancel) = &current_cancel {
                cancel.cancel();
              }
              if turn.is_none() {
                if let Some(reply) = shutdown_reply.take() {
                  let _ = reply.send(());
                }
                break;
              }
            }
          }
        }
        Some(control) = self.control.recv() => {
          let Control::Cancel { turn_id } = control;
          let matches = match turn_id {
            Some(id) => current_turn == Some(id),
            None => current_turn.is_some(),
          };
          if matches && let Some(cancel) = &current_cancel {
            cancel.cancel();
          }
        }
        result = poll_turn(&mut turn), if turn.is_some() => {
          let TurnResult {
            session,
            turn_id,
            outcome,
          } = result;

          self.session = Some(session);
          current_turn = None;
          current_cancel = None;
          turn = None;

          if let Some(id) = self.current_queued.take()
            && let Err(error) = self.repo.complete_queued_message(id).await
          {
            self.emit(ServerEvent::Error {
              request_id: None,
              session_id: Some(self.id),
              message: format!("Failed to complete queued message: {error}"),
            }).await;
          }

          let completed = matches!(&outcome, TurnOutcome::Completed { .. });
          let correlation = Correlation {
            session_id: self.id,
            turn_id,
          };
          self.emit(ServerEvent::TurnFinished {
            correlation,
            outcome,
          }).await;

          if completed {
            let usage = self.session.as_ref().and_then(|session| session.last_usage);
            let context_window = match self
              .session
              .as_ref()
              .and_then(|session| session.model.clone())
            {
              Some(model) => {
                ProviderManager::new(self.config.clone())
                  .context_window(&model)
                  .await
              }
              None => None,
            };
            self.emit(ServerEvent::Usage {
              correlation,
              usage,
              context_window,
            }).await;
          }

          if shutting_down {
            if let Some(reply) = shutdown_reply.take() {
              let _ = reply.send(());
            }
            break;
          }

          self.start_next_queued(
            &mut turn,
            &mut current_turn,
            &mut current_cancel,
          ).await;
        }
      }
    }

    let _ = self
      .events
      .send(SessionEvent::Stopped {
        session_id: self.id,
      })
      .await;
  }

  async fn start_turn(
    &mut self,
    start: TurnStart,
    turn: &mut Option<TurnFuture>,
    current_turn: &mut Option<Uuid>,
    current_cancel: &mut Option<CancellationToken>,
  ) {
    let TurnStart {
      request_id,
      client_id,
      mode,
      text,
    } = start;
    let Some(mut session) = self.session.take() else {
      return;
    };
    let turn_id = Uuid::now_v7();
    let cancel = CancellationToken::new();
    *current_turn = Some(turn_id);
    *current_cancel = Some(cancel.clone());

    self
      .emit(ServerEvent::TurnStarted {
        correlation: Correlation {
          session_id: self.id,
          turn_id,
        },
        client_id,
      })
      .await;

    let config = self.config.clone();
    let repo = self.repo.clone();
    let events = self.events.clone();
    let providers = ProviderManager::new(config.clone());

    *turn = Some(Box::pin(async move {
      let context = AgentContext {
        config: &config,
        providers: &providers,
        repo: &repo,
        events: &events,
        cancel: &cancel,
      };
      let outcome = run_turn(
        &mut session,
        turn_id,
        request_id,
        client_id,
        mode,
        text,
        &context,
      )
      .await;
      TurnResult {
        session,
        turn_id,
        outcome,
      }
    }));
  }

  async fn start_compact(
    &mut self,
    request_id: u64,
    client_id: Uuid,
    turn: &mut Option<TurnFuture>,
    current_turn: &mut Option<Uuid>,
    current_cancel: &mut Option<CancellationToken>,
  ) {
    let Some(mut session) = self.session.take() else {
      return;
    };
    let turn_id = Uuid::now_v7();
    let cancel = CancellationToken::new();
    *current_turn = Some(turn_id);
    *current_cancel = Some(cancel.clone());

    self
      .emit(ServerEvent::TurnStarted {
        correlation: Correlation {
          session_id: self.id,
          turn_id,
        },
        client_id,
      })
      .await;

    let config = self.config.clone();
    let repo = self.repo.clone();
    let events = self.events.clone();
    let providers = ProviderManager::new(config.clone());

    *turn = Some(Box::pin(async move {
      let context = AgentContext {
        config: &config,
        providers: &providers,
        repo: &repo,
        events: &events,
        cancel: &cancel,
      };
      let outcome =
        run_compact(&mut session, turn_id, request_id, &context).await;
      TurnResult {
        session,
        turn_id,
        outcome,
      }
    }));
  }

  async fn set_persona(&mut self, request_id: u64, name: &str) {
    let Some(mut session) = self.session.take() else {
      return;
    };
    let result = match PersonaManager::find(name) {
      Ok(persona) => {
        let persona_name = persona.name().to_string();
        match self.repo.set_persona(session.id, &persona_name).await {
          Ok(()) => {
            session.persona_manager.set_persona(persona);
            session.persona = persona_name.clone();
            if session.has_system_message() {
              let prompt = crate::prompt::system::build(
                &self.config,
                session.persona_manager.current(),
                &session.cwd,
              );
              session.replace_system_message(&prompt);
            }
            Ok(persona_name)
          }
          Err(e) => Err(e),
        }
      }
      Err(e) => Err(e),
    };
    self.session = Some(session);
    match result {
      Ok(persona) => {
        self
          .emit(ServerEvent::PersonaUpdated {
            request_id,
            session_id: Some(self.id),
            persona,
          })
          .await;
      }
      Err(e) => {
        self
          .emit(ServerEvent::Error {
            request_id: Some(request_id),
            session_id: Some(self.id),
            message: e.to_string(),
          })
          .await;
      }
    }
  }

  async fn set_model(&mut self, request_id: u64, model: Model) {
    let Some(mut session) = self.session.take() else {
      return;
    };
    let result = match self.repo.set_model(session.id, &model).await {
      Ok(()) => {
        session.model = Some(model.clone());
        Ok(())
      }
      Err(e) => Err(e),
    };
    self.session = Some(session);
    match result {
      Ok(()) => {
        self
          .emit(ServerEvent::ModelUpdated {
            request_id,
            session_id: Some(self.id),
            model,
          })
          .await;
      }
      Err(e) => {
        self
          .emit(ServerEvent::Error {
            request_id: Some(request_id),
            session_id: Some(self.id),
            message: e.to_string(),
          })
          .await;
      }
    }
  }

  async fn enqueue_prompt(&mut self, text: Option<String>) {
    let Some(text) = text else {
      return;
    };
    if let Err(e) = self.repo.insert_queued_message(self.id, &text).await {
      self
        .emit(ServerEvent::Error {
          request_id: None,
          session_id: Some(self.id),
          message: format!("Failed to queue message: {e}"),
        })
        .await;
      return;
    }
    self.emit_queue().await;
  }

  async fn start_next_queued(
    &mut self,
    turn: &mut Option<TurnFuture>,
    current_turn: &mut Option<Uuid>,
    current_cancel: &mut Option<CancellationToken>,
  ) {
    let next = match self.repo.claim_next_queued_message(self.id).await {
      Ok(next) => next,
      Err(e) => {
        self
          .emit(ServerEvent::Error {
            request_id: None,
            session_id: Some(self.id),
            message: format!("Failed to claim queued message: {e}"),
          })
          .await;
        return;
      }
    };
    let Some(message) = next else {
      return;
    };
    self.current_queued = Some(message.id);
    self.emit_queue().await;
    self
      .start_turn(
        TurnStart {
          request_id: None,
          client_id: Uuid::nil(),
          mode: PromptMode::Normal,
          text: Some(message.content),
        },
        turn,
        current_turn,
        current_cancel,
      )
      .await;
  }

  async fn emit_queue(&self) {
    let messages = self
      .repo
      .load_queued_messages(self.id)
      .await
      .unwrap_or_default();
    self
      .emit(ServerEvent::QueueUpdated {
        session_id: self.id,
        messages,
      })
      .await;
  }

  async fn emit(&self, event: ServerEvent) {
    let _ = self.events.send(SessionEvent::Event(event)).await;
  }
}

async fn poll_turn(turn: &mut Option<TurnFuture>) -> TurnResult {
  match turn {
    Some(future) => future.as_mut().await,
    None => std::future::pending().await,
  }
}
