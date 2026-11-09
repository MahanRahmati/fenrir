use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use axum::serve;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;

use fenrir_core::client::Client;
use fenrir_core::config::ServerConfig;
use fenrir_core::engine::broker::BrokerHandle;
use fenrir_core::engine::protocol::{ServerEvent, SessionSummary};
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

struct Adapter {
  client: Client,
  requests: Receiver<AdapterRequest>,
  pending: HashMap<u64, oneshot::Sender<Result<Vec<SessionSummary>>>>,
}

pub async fn run(broker: &BrokerHandle, config: ServerConfig) -> Result<()> {
  let cwd = std::env::current_dir()
    .map(|path| path.display().to_string())
    .unwrap_or_default();
  let client = Client::connect(broker, &cwd).await?;
  let (requests, inbox) = mpsc::channel(64);
  let handle = AdapterHandle { requests };
  let mut adapter = Adapter {
    client,
    requests: inbox,
    pending: HashMap::new(),
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
        event = self.client.recv() => {
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
        match self.client.list_sessions().await {
          Ok(request_id) => {
            self.pending.insert(request_id, reply);
          }
          Err(error) => {
            let _ = reply.send(Err(error));
          }
        }
      }
    }
  }

  fn handle_event(&mut self, event: ServerEvent) {
    vlog!("[server] event: {event:?}");
    match event {
      ServerEvent::SessionsListed {
        request_id,
        sessions,
      } => {
        if let Some(reply) = self.pending.remove(&request_id) {
          let _ = reply.send(Ok(sessions));
        }
      }
      ServerEvent::Error {
        request_id: Some(request_id),
        message,
        ..
      } => {
        if let Some(reply) = self.pending.remove(&request_id) {
          let _ = reply.send(Err(anyhow!(message)));
        }
      }
      _ => {}
    }
  }

  async fn shutdown(&mut self) {
    let _ = time::timeout(SHUTDOWN_TIMEOUT, self.client.shutdown()).await;
  }
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
