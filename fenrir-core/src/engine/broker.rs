use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::command::{Command, Effect};
use crate::config::Config;
use crate::engine::protocol::{
  ApprovalDecision, BrokerRequest, ClientRequest, PersonaInfo, PromptMode,
  ServerEvent, SessionEvent, SessionView,
};
use crate::engine::repo_actor::{RepoHandle, summary_of};
use crate::engine::session_actor::{
  Control, SessionActor, SessionHandle, SessionRequest,
};
use crate::persona_manager::PersonaManager;
use crate::provider_manager::{Model, ProviderManager};
use crate::session_manager::session::Session;

#[derive(Clone)]
pub struct BrokerHandle {
  requests: Sender<BrokerRequest>,
}

pub struct ClientHandle {
  pub id: Uuid,
  pub events: Receiver<ServerEvent>,
  requests: Sender<BrokerRequest>,
}

impl ClientHandle {
  pub async fn send(&self, request: ClientRequest) -> Result<()> {
    return self
      .requests
      .send(BrokerRequest::Client {
        client_id: self.id,
        request,
      })
      .await
      .context("Broker is not running");
  }
}

impl BrokerHandle {
  pub async fn connect(&self, cwd: &str) -> Result<ClientHandle> {
    let (events_tx, events_rx) = mpsc::channel(256);
    let (reply_tx, reply_rx) = oneshot::channel();
    self
      .requests
      .send(BrokerRequest::Connect {
        cwd: cwd.to_string(),
        events: events_tx,
        reply: reply_tx,
      })
      .await
      .context("Broker is not running")?;
    let id = reply_rx.await.context("Broker did not accept client")?;
    return Ok(ClientHandle {
      id,
      events: events_rx,
      requests: self.requests.clone(),
    });
  }
}

struct ClientState {
  cwd: String,
  events: Sender<ServerEvent>,
}

struct PendingApproval {
  event: ServerEvent,
  reply: oneshot::Sender<ApprovalDecision>,
}

struct Broker {
  config: Config,
  repo: RepoHandle,
  requests: Receiver<BrokerRequest>,
  session_events: Receiver<SessionEvent>,
  session_event_tx: Sender<SessionEvent>,
  clients: HashMap<Uuid, ClientState>,
  sessions: HashMap<Uuid, SessionHandle>,
  subscriptions: HashMap<Uuid, HashSet<Uuid>>,
  pending_approvals: HashMap<(Uuid, Uuid, String), PendingApproval>,
}

pub fn spawn(config: Config, repo: RepoHandle) -> BrokerHandle {
  let (request_tx, request_rx) = mpsc::channel(256);
  let (event_tx, event_rx) = mpsc::channel(512);
  let broker = Broker {
    config,
    repo,
    requests: request_rx,
    session_events: event_rx,
    session_event_tx: event_tx,
    clients: HashMap::new(),
    sessions: HashMap::new(),
    subscriptions: HashMap::new(),
    pending_approvals: HashMap::new(),
  };
  tokio::spawn(broker.run());
  return BrokerHandle {
    requests: request_tx,
  };
}

impl Broker {
  async fn run(mut self) {
    loop {
      tokio::select! {
        request = self.requests.recv() => {
          let Some(request) = request else {
            break;
          };
          match request {
            BrokerRequest::Connect { cwd, events, reply } => {
              let id = Uuid::now_v7();
              self.clients.insert(id, ClientState { cwd, events });
              let _ = reply.send(id);
              self.send_to(id, ServerEvent::Connected { client_id: id });
            }
            BrokerRequest::Client { client_id, request } => {
              if matches!(&request, ClientRequest::Shutdown) {
                self.shutdown().await;
                break;
              }
              if let Err(error) = self
                .handle_client_request(client_id, request)
                .await
              {
                self.send_to(
                  client_id,
                  ServerEvent::Error {
                    request_id: None,
                    session_id: None,
                    message: error.to_string(),
                  },
                );
              }
            }
          }
        }
        Some(event) = self.session_events.recv() => {
          self.handle_session_event(event);
        }
        else => break,
      }
    }

    for (_, handle) in self.sessions.drain() {
      let _ = handle
        .requests
        .send(SessionRequest::Shutdown { reply: None })
        .await;
    }
  }

  async fn handle_client_request(
    &mut self,
    client_id: Uuid,
    request: ClientRequest,
  ) -> Result<()> {
    match request {
      ClientRequest::Disconnect => {
        self.remove_client(client_id);
        return Ok(());
      }
      ClientRequest::CreateSession {
        request_id,
        persona,
        model,
      } => {
        let session = self
          .repo
          .create_session(
            persona,
            self.model_or_default(model),
            self.client_cwd(client_id)?,
          )
          .await?;
        let session_id = session.id;
        let summary = summary_of(&session);
        self.ensure_actor(session).await?;
        self
          .subscriptions
          .entry(session_id)
          .or_default()
          .insert(client_id);
        self.send_to(
          client_id,
          ServerEvent::SessionCreated {
            request_id,
            session: summary,
          },
        );
        return Ok(());
      }
      ClientRequest::LoadSession {
        request_id,
        session_id,
      } => {
        self
          .subscriptions
          .entry(session_id)
          .or_default()
          .insert(client_id);
        let session = match self.load_session(session_id).await {
          Ok(session) => session,
          Err(error) => {
            if let Some(subscribers) = self.subscriptions.get_mut(&session_id) {
              subscribers.remove(&client_id);
            }
            return Err(error);
          }
        };
        let view = view_of(&session);
        self.ensure_actor(session).await?;
        self.send_to(
          client_id,
          ServerEvent::SessionLoaded {
            request_id,
            session: view,
          },
        );
        self.send_queue(client_id, session_id).await;
        self.send_pending_approvals(client_id, session_id);
        return Ok(());
      }
      ClientRequest::ListSessions { request_id } => {
        let sessions = self.repo.list_sessions().await?;
        self.send_to(
          client_id,
          ServerEvent::SessionsListed {
            request_id,
            sessions,
          },
        );
        return Ok(());
      }
      ClientRequest::ListPersonas { request_id } => {
        self.send_to(
          client_id,
          ServerEvent::PersonasListed {
            request_id,
            personas: persona_infos(),
            current: None,
          },
        );
        return Ok(());
      }
      ClientRequest::ListModels {
        request_id,
        session_id,
      } => {
        let providers = ProviderManager::new(self.config.clone());
        let _ = session_id;
        self.send_to(
          client_id,
          ServerEvent::ModelsListed {
            request_id,
            models: providers.list_models(),
            current: None,
          },
        );
        return Ok(());
      }
      ClientRequest::Subscribe { session_id } => {
        self.ensure_session(session_id).await?;
        self
          .subscriptions
          .entry(session_id)
          .or_default()
          .insert(client_id);
        self.send_queue(client_id, session_id).await;
        self.send_pending_approvals(client_id, session_id);
        return Ok(());
      }
      ClientRequest::Unsubscribe { session_id } => {
        let orphaned = match self.subscriptions.get_mut(&session_id) {
          Some(set) => {
            set.remove(&client_id);
            set.is_empty()
          }
          None => false,
        };
        if orphaned {
          self
            .pending_approvals
            .retain(|(id, _, _), _| *id != session_id);
        }
        return Ok(());
      }
      ClientRequest::SendPrompt {
        request_id,
        session_id,
        mode,
        text,
      } => {
        let session_id = match session_id {
          Some(id) => id,
          None => {
            let session = self
              .repo
              .create_session(
                None,
                self.model_or_default(None),
                self.client_cwd(client_id)?,
              )
              .await?;
            let id = session.id;
            let summary = summary_of(&session);
            self.ensure_actor(session).await?;
            self.subscriptions.entry(id).or_default().insert(client_id);
            self.send_to(
              client_id,
              ServerEvent::SessionCreated {
                request_id,
                session: summary,
              },
            );
            id
          }
        };
        self.ensure_session(session_id).await?;
        self.require_subscription(client_id, session_id)?;
        let handle = self
          .sessions
          .get(&session_id)
          .context("Session actor is not running")?;
        handle
          .requests
          .send(SessionRequest::Turn {
            client_id,
            mode,
            text,
          })
          .await
          .context("Session actor is not running")?;
        return Ok(());
      }
      ClientRequest::RunCommand {
        request_id,
        session_id,
        command,
      } => {
        return self
          .run_command(request_id, client_id, session_id, command)
          .await;
      }
      ClientRequest::Compact {
        request_id,
        session_id,
      } => {
        self.ensure_session(session_id).await?;
        self.require_subscription(client_id, session_id)?;
        let handle = self
          .sessions
          .get(&session_id)
          .context("Session actor is not running")?;
        handle
          .requests
          .send(SessionRequest::Compact {
            request_id,
            client_id,
          })
          .await?;
        return Ok(());
      }
      ClientRequest::Cancel {
        session_id,
        turn_id,
      } => {
        self.require_subscription(client_id, session_id)?;
        let handle = self
          .sessions
          .get(&session_id)
          .context("Session actor is not running")?;
        handle.control.send(Control::Cancel { turn_id }).await?;
        return Ok(());
      }
      ClientRequest::ApproveTool {
        session_id,
        turn_id,
        tool_call_id,
        approved,
      } => {
        self.require_subscription(client_id, session_id)?;
        let key = (session_id, turn_id, tool_call_id);
        return match self.pending_approvals.remove(&key) {
          Some(pending) => {
            let decision = if approved {
              ApprovalDecision::Approved
            } else {
              ApprovalDecision::Denied
            };
            let _ = pending.reply.send(decision);
            Ok(())
          }
          None => {
            self.send_to(
              client_id,
              ServerEvent::Error {
                request_id: None,
                session_id: Some(session_id),
                message: "Approval request is already resolved or unknown."
                  .to_string(),
              },
            );
            Ok(())
          }
        };
      }
      ClientRequest::Shutdown => return Ok(()),
    }
  }

  async fn shutdown(&mut self) {
    for handle in self.sessions.values() {
      let _ = handle
        .requests
        .send(SessionRequest::Shutdown { reply: None })
        .await;
      let _ = handle.control.try_send(Control::Cancel { turn_id: None });
    }
    self.broadcast_all(ServerEvent::Exited);
  }

  async fn run_command(
    &mut self,
    request_id: u64,
    client_id: Uuid,
    session_id: Option<Uuid>,
    command: Command,
  ) -> Result<()> {
    match command.effect(session_id)? {
      Effect::CreateSession { persona, model } => {
        let session = self
          .repo
          .create_session(
            persona,
            self.model_or_default(model),
            self.client_cwd(client_id)?,
          )
          .await?;
        let id = session.id;
        let summary = summary_of(&session);
        self.ensure_actor(session).await?;
        self.subscriptions.entry(id).or_default().insert(client_id);
        self.send_to(
          client_id,
          ServerEvent::SessionCreated {
            request_id,
            session: summary,
          },
        );
        return Ok(());
      }
      Effect::ListSessions => {
        let sessions = self.repo.list_sessions().await?;
        self.send_to(
          client_id,
          ServerEvent::SessionsListed {
            request_id,
            sessions,
          },
        );
        return Ok(());
      }
      Effect::ListPersonas => {
        self.send_to(
          client_id,
          ServerEvent::PersonasListed {
            request_id,
            personas: persona_infos(),
            current: None,
          },
        );
        return Ok(());
      }
      Effect::ListModels => {
        let providers = ProviderManager::new(self.config.clone());
        self.send_to(
          client_id,
          ServerEvent::ModelsListed {
            request_id,
            models: providers.list_models(),
            current: None,
          },
        );
        return Ok(());
      }
      Effect::ShowHelp => {
        self.send_to(
          client_id,
          ServerEvent::CommandCatalog {
            request_id,
            commands: Command::catalog(),
          },
        );
        return Ok(());
      }
      Effect::StartReview { session_id } => {
        let session_id = match session_id {
          Some(id) => id,
          None => {
            let session = self
              .repo
              .create_session(
                None,
                self.model_or_default(None),
                self.client_cwd(client_id)?,
              )
              .await?;
            let id = session.id;
            let summary = summary_of(&session);
            self.ensure_actor(session).await?;
            self.subscriptions.entry(id).or_default().insert(client_id);
            self.send_to(
              client_id,
              ServerEvent::SessionCreated {
                request_id,
                session: summary,
              },
            );
            id
          }
        };
        self.ensure_session(session_id).await?;
        self.require_subscription(client_id, session_id)?;
        let handle = self
          .sessions
          .get(&session_id)
          .context("Session actor is not running")?;
        handle
          .requests
          .send(SessionRequest::Turn {
            client_id,
            mode: PromptMode::Review,
            text: None,
          })
          .await?;
        return Ok(());
      }
      Effect::Compact { session_id } => {
        self.ensure_session(session_id).await?;
        self.require_subscription(client_id, session_id)?;
        let handle = self
          .sessions
          .get(&session_id)
          .context("Session actor is not running")?;
        handle
          .requests
          .send(SessionRequest::Compact {
            request_id,
            client_id,
          })
          .await?;
        return Ok(());
      }
      Effect::Exit => {
        self.send_to(client_id, ServerEvent::Exited);
        return Ok(());
      }
    }
  }

  async fn load_session(&self, session_id: Uuid) -> Result<Session> {
    let mut session = self.repo.load_session(session_id).await?;
    if session.model.is_none()
      && let Some(model) = self.model_or_default(None)
    {
      self.repo.set_model(session_id, &model).await?;
      session.model = Some(model);
    }
    return Ok(session);
  }

  async fn ensure_session(&mut self, session_id: Uuid) -> Result<()> {
    if self.sessions.contains_key(&session_id) {
      return Ok(());
    }
    let session = self.load_session(session_id).await?;
    return self.ensure_actor(session).await;
  }

  async fn ensure_actor(&mut self, session: Session) -> Result<()> {
    let session_id = session.id;
    if self.sessions.contains_key(&session_id) {
      return Ok(());
    }
    let handle = SessionActor::spawn(
      session,
      self.config.clone(),
      self.repo.clone(),
      self.session_event_tx.clone(),
    );
    self.sessions.insert(session_id, handle);
    return Ok(());
  }

  fn handle_session_event(&mut self, event: SessionEvent) {
    match event {
      SessionEvent::Event(event) => {
        match &event {
          ServerEvent::TurnFinished {
            correlation,
            outcome: _,
          } => {
            self
              .pending_approvals
              .retain(|(session_id, turn_id, _), _| {
                *session_id != correlation.session_id
                  || *turn_id != correlation.turn_id
              });
          }
          ServerEvent::ToolApprovalResult {
            correlation,
            tool_call_id,
            status: _,
          } => {
            self.pending_approvals.remove(&(
              correlation.session_id,
              correlation.turn_id,
              tool_call_id.clone(),
            ));
          }
          _ => {}
        }
        self.broadcast_event(event);
      }
      SessionEvent::ApprovalRequest { event, reply } => {
        if let ServerEvent::ToolApprovalRequest {
          correlation,
          tool_call_id,
          tool_name: _,
          message: _,
        } = &event
        {
          let has_subscribers = self
            .subscriptions
            .get(&correlation.session_id)
            .is_some_and(|subscribers| !subscribers.is_empty());
          if !has_subscribers {
            drop(reply);
            return;
          }
          self.pending_approvals.insert(
            (
              correlation.session_id,
              correlation.turn_id,
              tool_call_id.clone(),
            ),
            PendingApproval {
              event: event.clone(),
              reply,
            },
          );
        }
        self.broadcast_event(event);
      }
      SessionEvent::Stopped { session_id } => {
        self.sessions.remove(&session_id);
        self.subscriptions.remove(&session_id);
        self
          .pending_approvals
          .retain(|(id, _, _), _| *id != session_id);
        self.broadcast_all(ServerEvent::SessionStopped { session_id });
      }
    }
  }

  async fn send_queue(&mut self, client_id: Uuid, session_id: Uuid) {
    let messages = self
      .repo
      .load_queued_messages(session_id)
      .await
      .unwrap_or_default();
    self.send_to(
      client_id,
      ServerEvent::QueueUpdated {
        session_id,
        messages,
      },
    );
  }

  fn send_pending_approvals(&mut self, client_id: Uuid, session_id: Uuid) {
    let events: Vec<ServerEvent> = self
      .pending_approvals
      .iter()
      .filter(|((id, _, _), _)| *id == session_id)
      .map(|(_, pending)| pending.event.clone())
      .collect();
    for event in events {
      self.send_to(client_id, event);
    }
  }

  fn broadcast_event(&mut self, event: ServerEvent) {
    match session_id_of(&event) {
      Some(id) => self.broadcast(id, event),
      None => self.broadcast_all(event),
    }
  }

  fn broadcast(&mut self, session_id: Uuid, event: ServerEvent) {
    let clients: Vec<Uuid> = self
      .subscriptions
      .get(&session_id)
      .map(|set| set.iter().copied().collect())
      .unwrap_or_default();
    let mut dead = Vec::new();
    for id in clients {
      if let Some(client) = self.clients.get(&id)
        && client.events.try_send(event.clone()).is_err()
      {
        dead.push(id);
      }
    }
    for id in dead {
      self.remove_client(id);
    }
  }

  fn broadcast_all(&mut self, event: ServerEvent) {
    let ids: Vec<Uuid> = self.clients.keys().copied().collect();
    for id in ids {
      self.send_to(id, event.clone());
    }
  }

  fn send_to(&mut self, id: Uuid, event: ServerEvent) {
    if let Some(client) = self.clients.get(&id) {
      let _ = client.events.try_send(event);
    }
  }

  fn model_or_default(&self, model: Option<Model>) -> Option<Model> {
    return model
      .or_else(|| ProviderManager::new(self.config.clone()).default_model());
  }

  fn client_cwd(&self, id: Uuid) -> Result<String> {
    return self
      .clients
      .get(&id)
      .map(|client| client.cwd.clone())
      .context("Unknown client");
  }

  fn remove_client(&mut self, id: Uuid) {
    self.clients.remove(&id);
    let mut orphaned = Vec::new();
    for (session_id, subscribers) in self.subscriptions.iter_mut() {
      subscribers.remove(&id);
      if subscribers.is_empty() {
        orphaned.push(*session_id);
      }
    }
    for session_id in orphaned {
      self
        .pending_approvals
        .retain(|(id, _, _), _| *id != session_id);
    }
  }

  fn require_subscription(
    &self,
    client_id: Uuid,
    session_id: Uuid,
  ) -> Result<()> {
    let subscribed = self
      .subscriptions
      .get(&session_id)
      .is_some_and(|set| set.contains(&client_id));
    if !subscribed {
      bail!("Client is not subscribed to session {session_id}");
    }
    return Ok(());
  }
}

fn persona_infos() -> Vec<PersonaInfo> {
  return PersonaManager::list()
    .into_iter()
    .map(|persona| PersonaInfo::from(&persona))
    .collect();
}

fn view_of(session: &Session) -> SessionView {
  return SessionView {
    summary: summary_of(session),
    persona: session.persona_manager.current().name().to_string(),
    model: session.model.clone(),
    messages: session.messages.clone(),
    summaries: session.summaries.clone(),
  };
}

fn session_id_of(event: &ServerEvent) -> Option<Uuid> {
  return match event {
    ServerEvent::SessionCreated { session, .. } => Some(session.id),
    ServerEvent::SessionLoaded { session, .. } => Some(session.summary.id),
    ServerEvent::SessionsListed { .. }
    | ServerEvent::PersonasListed { .. }
    | ServerEvent::ModelsListed { .. }
    | ServerEvent::CommandCatalog { .. }
    | ServerEvent::Connected { .. }
    | ServerEvent::Exited => None,
    ServerEvent::UserMessage { correlation, .. }
    | ServerEvent::TurnStarted { correlation, .. }
    | ServerEvent::Token { correlation, .. }
    | ServerEvent::ToolCall { correlation, .. }
    | ServerEvent::ToolApprovalRequest { correlation, .. }
    | ServerEvent::ToolApprovalResult { correlation, .. }
    | ServerEvent::ToolResult { correlation, .. }
    | ServerEvent::Usage { correlation, .. }
    | ServerEvent::Subagents { correlation, .. }
    | ServerEvent::TurnFinished { correlation, .. } => {
      Some(correlation.session_id)
    }
    ServerEvent::Compacted { session_id, .. }
    | ServerEvent::Prediction { session_id, .. }
    | ServerEvent::TitleUpdated { session_id, .. }
    | ServerEvent::QueueUpdated { session_id, .. }
    | ServerEvent::SessionStopped { session_id } => Some(*session_id),
    ServerEvent::Error { session_id, .. } => *session_id,
  };
}
