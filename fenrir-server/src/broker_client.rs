use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use tokio::sync::mpsc::Receiver;
use tokio::time;
use uuid::Uuid;

use fenrir_core::engine::broker::{BrokerHandle, ClientSender};
use fenrir_core::engine::protocol::{
  ClientRequest, PersonaInfo, ServerEvent, SessionSummary, SessionView,
};
use fenrir_core::provider_manager::Model;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

pub struct TempClient {
  sender: ClientSender,
  events: Receiver<ServerEvent>,
  next_request_id: u64,
}

impl TempClient {
  pub async fn connect(broker: &BrokerHandle, cwd: &str) -> Result<Self> {
    let (sender, events) = broker.connect(cwd).await?.into_parts();
    return Ok(Self {
      sender,
      events,
      next_request_id: 1,
    });
  }

  pub async fn list_sessions(
    &mut self,
    limit: Option<u32>,
  ) -> Result<Vec<SessionSummary>> {
    let request_id = self.take_request_id();
    self
      .sender
      .send(ClientRequest::ListSessions { request_id, limit })
      .await?;
    return self
      .await_event(request_id, |event| match event {
        ServerEvent::SessionsListed {
          request_id: id,
          sessions,
        } if id == request_id => Some(sessions),
        _ => None,
      })
      .await;
  }

  pub async fn list_personas(
    &mut self,
  ) -> Result<(Vec<PersonaInfo>, Option<String>)> {
    let request_id = self.take_request_id();
    self
      .sender
      .send(ClientRequest::ListPersonas { request_id })
      .await?;
    return self
      .await_event(request_id, |event| match event {
        ServerEvent::PersonasListed {
          request_id: id,
          personas,
          current,
        } if id == request_id => Some((personas, current)),
        _ => None,
      })
      .await;
  }

  pub async fn list_models(&mut self) -> Result<(Vec<Model>, Option<Model>)> {
    let request_id = self.take_request_id();
    self
      .sender
      .send(ClientRequest::ListModels {
        request_id,
        session_id: None,
      })
      .await?;
    return self
      .await_event(request_id, |event| match event {
        ServerEvent::ModelsListed {
          request_id: id,
          models,
          current,
        } if id == request_id => Some((models, current)),
        _ => None,
      })
      .await;
  }

  pub async fn load_session(
    &mut self,
    session_id: Uuid,
  ) -> Result<SessionView> {
    let request_id = self.take_request_id();
    self
      .sender
      .send(ClientRequest::LoadSession {
        request_id,
        session_id,
      })
      .await?;
    return self
      .await_event(request_id, |event| match event {
        ServerEvent::SessionLoaded {
          request_id: id,
          session,
        } if id == request_id => Some(session),
        _ => None,
      })
      .await;
  }

  pub async fn disconnect(&mut self) {
    let _ = self.sender.send(ClientRequest::Disconnect).await;
  }

  pub async fn shutdown(self) {
    let _ = self.sender.send(ClientRequest::Shutdown).await;
  }

  async fn await_event<T>(
    &mut self,
    request_id: u64,
    mut map: impl FnMut(ServerEvent) -> Option<T>,
  ) -> Result<T> {
    loop {
      match self.recv().await? {
        ServerEvent::Exited => bail!("Broker exited"),
        ServerEvent::Error {
          request_id: Some(id),
          message,
          ..
        } if id == request_id => bail!("{message}"),
        event => {
          if let Some(value) = map(event) {
            return Ok(value);
          }
        }
      }
    }
  }

  async fn recv(&mut self) -> Result<ServerEvent> {
    return match time::timeout(RESPONSE_TIMEOUT, self.events.recv()).await {
      Ok(Some(event)) => Ok(event),
      Ok(None) => Err(anyhow!("Broker connection closed")),
      Err(_) => Err(anyhow!("Timed out waiting for broker response")),
    };
  }

  fn take_request_id(&mut self) -> u64 {
    let request_id = self.next_request_id;
    self.next_request_id += 1;
    return request_id;
  }
}
