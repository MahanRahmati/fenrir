use anyhow::Result;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::config::Config;
use crate::engine::protocol::SessionSummary;
use crate::provider_manager::Model;
use crate::session_manager::repository::SessionRepository;
use crate::session_manager::session::{Message, QueuedMessage, Session};
use crate::session_manager::summary::Summary;
use crate::session_manager::tool_approval::{
  ToolApprovalRequest, ToolApprovalStatus,
};

#[derive(Clone)]
pub struct RepoHandle {
  tx: Sender<RepoRequest>,
}

pub enum RepoRequest {
  CreateSession {
    parent_id: Option<Uuid>,
    persona: Option<String>,
    model: Option<Model>,
    cwd: String,
    reply: oneshot::Sender<Result<Session>>,
  },
  LoadSession {
    id: Uuid,
    reply: oneshot::Sender<Result<Session>>,
  },
  ListSessions {
    reply: oneshot::Sender<Result<Vec<SessionSummary>>>,
  },
  Summary {
    id: Uuid,
    reply: oneshot::Sender<Result<SessionSummary>>,
  },
  InsertMessage {
    session_id: Uuid,
    message: Message,
    reply: oneshot::Sender<Result<()>>,
  },
  InsertSummary {
    session_id: Uuid,
    summary: Summary,
    reply: oneshot::Sender<Result<()>>,
  },
  UpdateTitle {
    session_id: Uuid,
    title: String,
    reply: oneshot::Sender<Result<()>>,
  },
  SetPersona {
    session_id: Uuid,
    persona: String,
    reply: oneshot::Sender<Result<()>>,
  },
  SetModel {
    session_id: Uuid,
    model: Model,
    reply: oneshot::Sender<Result<()>>,
  },
  InsertQueuedMessage {
    session_id: Uuid,
    content: String,
    reply: oneshot::Sender<Result<QueuedMessage>>,
  },
  LoadQueuedMessages {
    session_id: Uuid,
    reply: oneshot::Sender<Result<Vec<QueuedMessage>>>,
  },
  HasQueuedMessages {
    session_id: Uuid,
    reply: oneshot::Sender<Result<bool>>,
  },
  ClaimNextQueuedMessage {
    session_id: Uuid,
    reply: oneshot::Sender<Result<Option<QueuedMessage>>>,
  },
  CompleteQueuedMessage {
    id: Uuid,
    reply: oneshot::Sender<Result<()>>,
  },
  ResetProcessingQueuedMessages {
    session_id: Uuid,
    reply: oneshot::Sender<Result<()>>,
  },
  InsertToolApproval {
    request: ToolApprovalRequest,
    reply: oneshot::Sender<Result<()>>,
  },
  LoadPendingToolApprovals {
    session_id: Uuid,
    reply: oneshot::Sender<Result<Vec<ToolApprovalRequest>>>,
  },
  ResolveToolApproval {
    tool_call_id: String,
    status: ToolApprovalStatus,
    reply: oneshot::Sender<Result<()>>,
  },
}

impl RepoHandle {
  pub async fn create_session(
    &self,
    persona: Option<String>,
    model: Option<Model>,
    cwd: String,
  ) -> Result<Session> {
    return self.create_child_session(None, persona, model, cwd).await;
  }

  pub async fn create_child_session(
    &self,
    parent_id: Option<Uuid>,
    persona: Option<String>,
    model: Option<Model>,
    cwd: String,
  ) -> Result<Session> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::CreateSession {
        parent_id,
        persona,
        model,
        cwd,
        reply,
      })
      .await?;
    return rx.await?;
  }

  pub async fn load_session(&self, id: Uuid) -> Result<Session> {
    let (reply, rx) = oneshot::channel();
    self.tx.send(RepoRequest::LoadSession { id, reply }).await?;
    return rx.await?;
  }

  pub async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
    let (reply, rx) = oneshot::channel();
    self.tx.send(RepoRequest::ListSessions { reply }).await?;
    return rx.await?;
  }

  pub async fn summary(&self, id: Uuid) -> Result<SessionSummary> {
    let (reply, rx) = oneshot::channel();
    self.tx.send(RepoRequest::Summary { id, reply }).await?;
    return rx.await?;
  }

  pub async fn insert_message(
    &self,
    session_id: Uuid,
    message: Option<Message>,
  ) -> Result<()> {
    let Some(message) = message else {
      return Ok(());
    };
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::InsertMessage {
        session_id,
        message,
        reply,
      })
      .await?;
    return rx.await?;
  }

  pub async fn insert_summary(
    &self,
    session_id: Uuid,
    summary: Summary,
  ) -> Result<()> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::InsertSummary {
        session_id,
        summary,
        reply,
      })
      .await?;
    return rx.await?;
  }

  pub async fn update_title(
    &self,
    session_id: Uuid,
    title: &str,
  ) -> Result<()> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::UpdateTitle {
        session_id,
        title: title.to_string(),
        reply,
      })
      .await?;
    return rx.await?;
  }

  pub async fn set_persona(
    &self,
    session_id: Uuid,
    persona: &str,
  ) -> Result<()> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::SetPersona {
        session_id,
        persona: persona.to_string(),
        reply,
      })
      .await?;
    return rx.await?;
  }

  pub async fn set_model(&self, session_id: Uuid, model: &Model) -> Result<()> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::SetModel {
        session_id,
        model: model.clone(),
        reply,
      })
      .await?;
    return rx.await?;
  }

  pub async fn insert_queued_message(
    &self,
    session_id: Uuid,
    content: &str,
  ) -> Result<QueuedMessage> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::InsertQueuedMessage {
        session_id,
        content: content.to_string(),
        reply,
      })
      .await?;
    return rx.await?;
  }

  pub async fn load_queued_messages(
    &self,
    session_id: Uuid,
  ) -> Result<Vec<QueuedMessage>> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::LoadQueuedMessages { session_id, reply })
      .await?;
    return rx.await?;
  }

  pub async fn has_queued_messages(&self, session_id: Uuid) -> Result<bool> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::HasQueuedMessages { session_id, reply })
      .await?;
    return rx.await?;
  }

  pub async fn claim_next_queued_message(
    &self,
    session_id: Uuid,
  ) -> Result<Option<QueuedMessage>> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::ClaimNextQueuedMessage { session_id, reply })
      .await?;
    return rx.await?;
  }

  pub async fn complete_queued_message(&self, id: Uuid) -> Result<()> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::CompleteQueuedMessage { id, reply })
      .await?;
    return rx.await?;
  }

  pub async fn reset_processing_queued_messages(
    &self,
    session_id: Uuid,
  ) -> Result<()> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::ResetProcessingQueuedMessages { session_id, reply })
      .await?;
    return rx.await?;
  }

  pub async fn insert_tool_approval(
    &self,
    request: ToolApprovalRequest,
  ) -> Result<()> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::InsertToolApproval { request, reply })
      .await?;
    return rx.await?;
  }

  pub async fn load_pending_tool_approvals(
    &self,
    session_id: Uuid,
  ) -> Result<Vec<ToolApprovalRequest>> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::LoadPendingToolApprovals { session_id, reply })
      .await?;
    return rx.await?;
  }

  pub async fn resolve_tool_approval(
    &self,
    tool_call_id: &str,
    status: ToolApprovalStatus,
  ) -> Result<()> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(RepoRequest::ResolveToolApproval {
        tool_call_id: tool_call_id.to_string(),
        status,
        reply,
      })
      .await?;
    return rx.await?;
  }
}

pub struct RepoActor {
  repo: SessionRepository,
  config: Config,
  inbox: Receiver<RepoRequest>,
}

impl RepoActor {
  pub fn spawn(repo: SessionRepository, config: Config) -> RepoHandle {
    let (tx, rx) = mpsc::channel(256);
    let actor = RepoActor {
      repo,
      config,
      inbox: rx,
    };
    tokio::spawn(actor.run());
    return RepoHandle { tx };
  }

  async fn run(mut self) {
    while let Some(request) = self.inbox.recv().await {
      self.handle(request).await;
    }
  }

  async fn handle(&mut self, request: RepoRequest) {
    match request {
      RepoRequest::CreateSession {
        parent_id,
        persona,
        model,
        cwd,
        reply,
      } => {
        let mut session = Session::new(parent_id, model, &self.config, &cwd);
        if let Some(name) = persona {
          if let Err(e) = session.persona_manager.set(&name) {
            let _ = reply.send(Err(e));
            return;
          }
          session.persona =
            session.persona_manager.current().name().to_string();
        }
        let result = self.repo.insert_session(&session).await.map(|_| session);
        let _ = reply.send(result);
      }
      RepoRequest::LoadSession { id, reply } => {
        let config = &self.config;
        let result = self
          .repo
          .load(id, |row| Session::from_row(row, config, None))
          .await;
        let _ = reply.send(result);
      }
      RepoRequest::ListSessions { reply } => {
        let result = self.repo.list_summaries().await;
        let _ = reply.send(result);
      }
      RepoRequest::Summary { id, reply } => {
        let config = &self.config;
        let result = self
          .repo
          .load(id, |row| Session::from_row(row, config, None))
          .await
          .map(|session| summary_of(&session));
        let _ = reply.send(result);
      }
      RepoRequest::InsertMessage {
        session_id,
        message,
        reply,
      } => {
        let result = self.repo.insert_message(session_id, &message).await;
        let _ = reply.send(result);
      }
      RepoRequest::InsertSummary {
        session_id,
        summary,
        reply,
      } => {
        let result = self.repo.insert_summary(session_id, &summary).await;
        let _ = reply.send(result);
      }
      RepoRequest::UpdateTitle {
        session_id,
        title,
        reply,
      } => {
        let result = self.repo.update_title(session_id, &title).await;
        let _ = reply.send(result);
      }
      RepoRequest::SetPersona {
        session_id,
        persona,
        reply,
      } => {
        let result = self.repo.set_persona(session_id, &persona).await;
        let _ = reply.send(result);
      }
      RepoRequest::SetModel {
        session_id,
        model,
        reply,
      } => {
        let result = self.repo.set_model(session_id, &model).await;
        let _ = reply.send(result);
      }
      RepoRequest::InsertQueuedMessage {
        session_id,
        content,
        reply,
      } => {
        let result =
          self.repo.insert_queued_message(session_id, &content).await;
        let _ = reply.send(result);
      }
      RepoRequest::LoadQueuedMessages { session_id, reply } => {
        let result = self.repo.load_queued_messages(session_id).await;
        let _ = reply.send(result);
      }
      RepoRequest::HasQueuedMessages { session_id, reply } => {
        let result = self.repo.has_queued_messages(session_id).await;
        let _ = reply.send(result);
      }
      RepoRequest::ClaimNextQueuedMessage { session_id, reply } => {
        let result = self.repo.claim_next_queued_message(session_id).await;
        let _ = reply.send(result);
      }
      RepoRequest::CompleteQueuedMessage { id, reply } => {
        let result = self.repo.complete_queued_message(id).await;
        let _ = reply.send(result);
      }
      RepoRequest::ResetProcessingQueuedMessages { session_id, reply } => {
        let result =
          self.repo.reset_processing_queued_messages(session_id).await;
        let _ = reply.send(result);
      }
      RepoRequest::InsertToolApproval { request, reply } => {
        let result = self.repo.insert_tool_approval(&request).await;
        let _ = reply.send(result);
      }
      RepoRequest::LoadPendingToolApprovals { session_id, reply } => {
        let result = self.repo.load_pending_tool_approvals(session_id).await;
        let _ = reply.send(result);
      }
      RepoRequest::ResolveToolApproval {
        tool_call_id,
        status,
        reply,
      } => {
        let result =
          self.repo.resolve_tool_approval(&tool_call_id, status).await;
        let _ = reply.send(result);
      }
    }
  }
}

pub fn summary_of(session: &Session) -> SessionSummary {
  return SessionSummary {
    id: session.id,
    parent_id: session.parent_id,
    title: session.title.clone(),
    created_at: session.created_at,
    last_interaction: session.last_interaction,
  };
}
