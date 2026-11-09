use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use axum::serve;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;

use fenrir_core::config::ServerConfig;
use fenrir_core::engine::broker::{BrokerHandle, ClientHandle};
use fenrir_core::engine::protocol::{
  ClientRequest, ServerEvent, SessionSummary,
};
use fenrir_core::vlog;
use tokio::time;

use crate::routes;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

enum AdapterRequest {
  ListSessions {
    reply: oneshot::Sender<Result<Vec<SessionSummary>>>,
  },
}

enum Pending {
  ListSessions {
    reply: oneshot::Sender<Result<Vec<SessionSummary>>>,
  },
}

struct Adapter {
  client: ClientHandle,
  requests: Receiver<AdapterRequest>,
  pending: HashMap<u64, Pending>,
  next_request_id: u64,
}

pub async fn run(broker: &BrokerHandle, config: ServerConfig) -> Result<()> {
  let cwd = std::env::current_dir()
    .map(|path| path.display().to_string())
    .unwrap_or_default();
  let client = broker.connect(&cwd).await?;
  let (requests, inbox) = mpsc::channel(64);
  let handle = AdapterHandle { requests };
  let mut adapter = Adapter {
    client,
    requests: inbox,
    pending: HashMap::new(),
    next_request_id: 1,
  };
  let app = routes::router().with_state(handle);

  let address = format!("{}:{}", config.host, config.port);
  let listener = TcpListener::bind(&address)
    .await
    .context(format!("Failed to bind server to '{address}'"))?;
  let local_address = listener
    .local_addr()
    .context("Failed to read server address")?;
  println!("Fenrir server listening on http://{local_address}");

  let server = serve(listener, app).with_graceful_shutdown(shutdown_signal());
  tokio::select! {
    result = server => result.context("Server failed")?,
    _ = adapter.run() => {}
  }

  adapter.shutdown().await;
  return Ok(());
}

#[derive(Clone)]
pub struct AdapterHandle {
  requests: Sender<AdapterRequest>,
}

impl AdapterHandle {
  pub async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
    let (reply, response) = oneshot::channel();
    self
      .requests
      .send(AdapterRequest::ListSessions { reply })
      .await
      .context("Adapter is not running")?;
    return match time::timeout(RESPONSE_TIMEOUT, response).await {
      Ok(result) => result.context("Adapter response channel closed")?,
      Err(_) => Err(anyhow!("Timed out waiting for broker response")),
    };
  }
}

impl Adapter {
  async fn run(&mut self) {
    loop {
      tokio::select! {
        request = self.requests.recv() => {
          let Some(request) = request else {
            break;
          };
          self.handle_request(request).await;
        }
        event = self.client.events.recv() => {
          let Some(event) = event else {
            break;
          };
          self.handle_event(event);
        }
      }
    }
  }

  async fn handle_request(&mut self, request: AdapterRequest) {
    match request {
      AdapterRequest::ListSessions { reply } => {
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        self
          .pending
          .insert(request_id, Pending::ListSessions { reply });
        if let Err(error) = self
          .client
          .send(ClientRequest::ListSessions { request_id })
          .await
        {
          self.fail(request_id, error);
        }
      }
    }
  }

  fn handle_event(&mut self, event: ServerEvent) {
    vlog!("[server] event: {event:?}");
    let Some(request_id) = event_request_id(&event) else {
      return;
    };
    let Some(pending) = self.pending.remove(&request_id) else {
      return;
    };
    match pending {
      Pending::ListSessions { reply } => {
        let result = match event {
          ServerEvent::SessionsListed { sessions, .. } => Ok(sessions),
          ServerEvent::Error { message, .. } => Err(anyhow!(message)),
          other => {
            Err(anyhow!("Unexpected response to list sessions: {other:?}"))
          }
        };
        let _ = reply.send(result);
      }
    }
  }

  fn fail(&mut self, request_id: u64, error: anyhow::Error) {
    let Some(pending) = self.pending.remove(&request_id) else {
      return;
    };
    match pending {
      Pending::ListSessions { reply } => {
        let _ = reply.send(Err(error));
      }
    }
  }

  async fn shutdown(&mut self) {
    let _ = time::timeout(
      SHUTDOWN_TIMEOUT,
      self.client.send(ClientRequest::Shutdown),
    )
    .await;
  }
}

fn event_request_id(event: &ServerEvent) -> Option<u64> {
  return match event {
    ServerEvent::SessionCreated { request_id, .. }
    | ServerEvent::SessionLoaded { request_id, .. }
    | ServerEvent::SessionsListed { request_id, .. }
    | ServerEvent::PersonasListed { request_id, .. }
    | ServerEvent::ModelsListed { request_id, .. }
    | ServerEvent::PersonaUpdated { request_id, .. }
    | ServerEvent::ModelUpdated { request_id, .. }
    | ServerEvent::CommandCatalog { request_id, .. } => Some(*request_id),
    ServerEvent::Error { request_id, .. } => *request_id,
    _ => None,
  };
}

async fn shutdown_signal() {
  let ctrl_c = async {
    let _ = tokio::signal::ctrl_c().await;
  };

  let terminate = async {
    if let Ok(mut interrupt) = signal(SignalKind::terminate()) {
      interrupt.recv().await;
    }
  };

  tokio::select! {
    _ = ctrl_c => {}
    _ = terminate => {}
  }
}
