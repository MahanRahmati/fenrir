mod state;

pub use state::{ClientState, PendingApproval, PendingRequest, RequestKind};

use anyhow::{Result, bail};
use uuid::Uuid;

use crate::command::Command;
use crate::engine::broker::{BrokerHandle, ClientHandle};
use crate::engine::protocol::{ClientRequest, PromptMode, ServerEvent};
use crate::provider_manager::Model;

pub struct Client {
  handle: ClientHandle,
  state: ClientState,
  next_request_id: u64,
}

impl Client {
  pub async fn connect(broker: &BrokerHandle, cwd: &str) -> Result<Self> {
    let handle = broker.connect(cwd).await?;
    let client_id = handle.id;
    return Ok(Self {
      handle,
      state: ClientState::new(client_id),
      next_request_id: 1,
    });
  }

  pub fn state(&self) -> &ClientState {
    return &self.state;
  }

  pub fn client_id(&self) -> Uuid {
    return self.handle.id;
  }

  pub fn take_next_approval(&mut self) -> Option<PendingApproval> {
    return self.state.take_next_approval();
  }

  pub async fn create_session(
    &mut self,
    persona: Option<String>,
    model: Option<Model>,
  ) -> Result<u64> {
    return self
      .send_tracked(RequestKind::CreateSession, None, false, |request_id| {
        ClientRequest::CreateSession {
          request_id,
          persona,
          model,
        }
      })
      .await;
  }

  pub async fn load_session(&mut self, session_id: Uuid) -> Result<u64> {
    return self
      .send_tracked(
        RequestKind::LoadSession,
        Some(session_id),
        false,
        |request_id| ClientRequest::LoadSession {
          request_id,
          session_id,
        },
      )
      .await;
  }

  pub async fn list_sessions(&mut self) -> Result<u64> {
    return self
      .send_tracked(RequestKind::ListSessions, None, false, |request_id| {
        ClientRequest::ListSessions { request_id }
      })
      .await;
  }

  pub async fn list_personas(&mut self) -> Result<u64> {
    return self
      .send_tracked(RequestKind::ListPersonas, None, false, |request_id| {
        ClientRequest::ListPersonas { request_id }
      })
      .await;
  }

  pub async fn list_models(&mut self, session_id: Option<Uuid>) -> Result<u64> {
    return self
      .send_tracked(RequestKind::ListModels, session_id, false, |request_id| {
        ClientRequest::ListModels {
          request_id,
          session_id,
        }
      })
      .await;
  }

  pub async fn set_persona(&mut self, name: impl Into<String>) -> Result<u64> {
    let session_id = self.state.visible_session();
    return self.set_persona_for(session_id, name).await;
  }

  pub async fn set_persona_for(
    &mut self,
    session_id: Option<Uuid>,
    name: impl Into<String>,
  ) -> Result<u64> {
    let name = name.into();
    return self
      .send_tracked(RequestKind::SetPersona, session_id, false, |request_id| {
        ClientRequest::SetPersona {
          request_id,
          session_id,
          name,
        }
      })
      .await;
  }

  pub async fn set_model(&mut self, model: Model) -> Result<u64> {
    let session_id = self.state.visible_session();
    return self.set_model_for(session_id, model).await;
  }

  pub async fn set_model_for(
    &mut self,
    session_id: Option<Uuid>,
    model: Model,
  ) -> Result<u64> {
    return self
      .send_tracked(RequestKind::SetModel, session_id, false, |request_id| {
        ClientRequest::SetModel {
          request_id,
          session_id,
          model,
        }
      })
      .await;
  }

  pub async fn send_prompt(&mut self, text: impl Into<String>) -> Result<u64> {
    let text = text.into();
    let session_id = self.state.visible_session();
    if session_id.is_none() {
      self.state.creating_session = true;
    }
    let result = self
      .send_tracked(RequestKind::SendPrompt, session_id, true, |request_id| {
        ClientRequest::SendPrompt {
          request_id,
          session_id,
          mode: PromptMode::Normal,
          text: Some(text),
        }
      })
      .await;
    if result.is_err() {
      self.state.creating_session = false;
    }
    return result;
  }

  pub async fn run_command(&mut self, command: Command) -> Result<u64> {
    let session_id = self.state.visible_session();
    let starts_turn = matches!(command, Command::Compact | Command::Review);
    return self
      .send_tracked(
        RequestKind::RunCommand,
        session_id,
        starts_turn,
        |request_id| ClientRequest::RunCommand {
          request_id,
          session_id,
          command,
        },
      )
      .await;
  }

  pub async fn compact(&mut self) -> Result<u64> {
    let Some(session_id) = self.state.visible_session() else {
      bail!("No session selected");
    };
    return self
      .send_tracked(
        RequestKind::Compact,
        Some(session_id),
        true,
        |request_id| ClientRequest::Compact {
          request_id,
          session_id,
        },
      )
      .await;
  }

  pub async fn predict(&mut self, session_id: Uuid) -> Result<()> {
    return self
      .handle
      .send(ClientRequest::Predict { session_id })
      .await;
  }

  pub async fn cancel(&mut self, turn_id: Option<Uuid>) -> Result<()> {
    let Some(session_id) = self.state.visible_session() else {
      return Ok(());
    };
    return self
      .handle
      .send(ClientRequest::Cancel {
        session_id,
        turn_id,
      })
      .await;
  }

  pub async fn approve_tool(
    &mut self,
    session_id: Uuid,
    turn_id: Uuid,
    tool_call_id: String,
    approved: bool,
  ) -> Result<()> {
    return self
      .handle
      .send(ClientRequest::ApproveTool {
        session_id,
        turn_id,
        tool_call_id,
        approved,
      })
      .await;
  }

  pub async fn subscribe(&mut self, session_id: Uuid) -> Result<()> {
    self.state.ensure_session(session_id);
    return self
      .handle
      .send(ClientRequest::Subscribe { session_id })
      .await;
  }

  pub async fn unsubscribe(&mut self, session_id: Uuid) -> Result<()> {
    self
      .handle
      .send(ClientRequest::Unsubscribe { session_id })
      .await?;
    self.state.forget_session(session_id);
    return Ok(());
  }

  pub async fn shutdown(&mut self) -> Result<()> {
    return self.handle.send(ClientRequest::Shutdown).await;
  }

  pub async fn disconnect(&mut self) -> Result<()> {
    return self.handle.send(ClientRequest::Disconnect).await;
  }

  pub fn apply_event(&mut self, event: ServerEvent) -> Option<ServerEvent> {
    return self.state.apply(event);
  }

  pub async fn recv(&mut self) -> Option<ServerEvent> {
    loop {
      let previous_session = self.state.visible_session();
      let event = self.handle.events.recv().await?;
      let Some(update) = self.state.apply(event) else {
        continue;
      };
      if let ServerEvent::SessionCreated { session, .. }
      | ServerEvent::SessionLoaded { session, .. } = &update
        && let Some(previous_session) = previous_session
        && previous_session != session.summary.id
      {
        let _ = self
          .handle
          .send(ClientRequest::Unsubscribe {
            session_id: previous_session,
          })
          .await;
        self.state.forget_session(previous_session);
      }
      return Some(update);
    }
  }

  pub async fn recv_all(&mut self) -> Option<ServerEvent> {
    let event = self.handle.events.recv().await?;
    return Some(self.state.apply_all(event));
  }

  fn take_request_id(&mut self) -> u64 {
    let request_id = self.next_request_id;
    self.next_request_id += 1;
    return request_id;
  }

  async fn send_tracked<F>(
    &mut self,
    kind: RequestKind,
    session_id: Option<Uuid>,
    starts_turn: bool,
    build: F,
  ) -> Result<u64>
  where
    F: FnOnce(u64) -> ClientRequest,
  {
    let request_id = self.take_request_id();
    self.state.insert_pending(
      request_id,
      PendingRequest {
        kind,
        session_id,
        starts_turn,
      },
    );
    if let Err(error) = self.handle.send(build(request_id)).await {
      self.state.remove_pending(request_id);
      return Err(error);
    }
    return Ok(request_id);
  }
}
