use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use axum::serve;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::time;

use fenrir_core::client::Client;
use fenrir_core::config::ServerConfig;
use fenrir_core::engine::broker::BrokerHandle;
use fenrir_core::engine::protocol::{ServerEvent, SessionSummary};

use crate::routes;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct AdapterHandle {
  broker: BrokerHandle,
  cwd: String,
}

pub async fn run(broker: &BrokerHandle, config: ServerConfig) -> Result<()> {
  let cwd = std::env::current_dir()
    .map(|path| path.display().to_string())
    .unwrap_or_default();
  let handle = AdapterHandle {
    broker: broker.clone(),
    cwd,
  };
  let app = routes::router().with_state(handle.clone());

  let address = format!("{}:{}", config.host, config.port);
  let listener = TcpListener::bind(&address)
    .await
    .context(format!("Failed to bind server to '{address}'"))?;
  let local_address = listener
    .local_addr()
    .context("Failed to read server address")?;
  println!("Fenrir server listening on http://{local_address}");

  let shutdown = async move {
    shutdown_signal().await;
    handle.shutdown().await;
  };
  let server = serve(listener, app).with_graceful_shutdown(shutdown);
  server.await.context("Server failed")?;
  return Ok(());
}

impl AdapterHandle {
  pub async fn connect(&self) -> Result<Client> {
    return Client::connect(&self.broker, &self.cwd).await;
  }

  pub async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
    let mut client = self.connect().await?;
    let result = list_sessions_with(&mut client).await;
    let _ = client.disconnect().await;
    return result;
  }

  async fn shutdown(&self) {
    let Ok(mut client) = self.connect().await else {
      return;
    };
    let _ = time::timeout(SHUTDOWN_TIMEOUT, client.shutdown()).await;
  }
}

async fn list_sessions_with(
  client: &mut Client,
) -> Result<Vec<SessionSummary>> {
  let request_id = client.list_sessions().await?;
  loop {
    match next_event(client).await? {
      ServerEvent::SessionsListed {
        request_id: id,
        sessions,
      } if id == request_id => return Ok(sessions),
      ServerEvent::Error {
        request_id: Some(id),
        message,
        ..
      } if id == request_id => bail!("{message}"),
      _ => {}
    }
  }
}

pub async fn next_event(client: &mut Client) -> Result<ServerEvent> {
  return match time::timeout(RESPONSE_TIMEOUT, client.recv()).await {
    Ok(Some(event)) => Ok(event),
    Ok(None) => Err(anyhow!("Broker connection closed")),
    Err(_) => Err(anyhow!("Timed out waiting for broker response")),
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
