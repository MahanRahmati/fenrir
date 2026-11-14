use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::serve;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::oneshot;
use tokio::time;

use fenrir_core::config::ServerConfig;
use fenrir_core::engine::broker::{BrokerHandle, ClientSender};
use fenrir_core::engine::protocol::SessionSummary;

use crate::broker_client::TempClient;
use crate::connections::{ConnectionsHandle, EventsReply};
use crate::routes;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct AdapterHandle {
  broker: BrokerHandle,
  cwd: String,
  connections: ConnectionsHandle,
}

pub struct ServerHandle {
  pub address: SocketAddr,
  stop: Option<oneshot::Sender<()>>,
  done: oneshot::Receiver<()>,
}

impl ServerHandle {
  pub async fn shutdown(mut self) {
    if let Some(stop) = self.stop.take() {
      let _ = stop.send(());
    }
    let _ = self.done.await;
  }
}

pub async fn spawn(
  broker: &BrokerHandle,
  config: ServerConfig,
) -> Result<ServerHandle> {
  let handle = adapter(broker);
  let app = routes::router().with_state(handle.clone());

  let address = format!("{}:{}", config.host, config.port);
  let listener = TcpListener::bind(&address)
    .await
    .context(format!("Failed to bind server to '{address}'"))?;
  let local_address = listener
    .local_addr()
    .context("Failed to read server address")?;

  let (stop, stopped) = oneshot::channel();
  let (done, finished) = oneshot::channel();
  tokio::spawn(async move {
    let shutdown = async move {
      let _ = stopped.await;
    };
    let server = serve(listener, app).with_graceful_shutdown(shutdown);
    if let Err(error) = server.await {
      eprintln!("Server failed: {error}");
    }
    handle.shutdown().await;
    let _ = done.send(());
  });

  return Ok(ServerHandle {
    address: local_address,
    stop: Some(stop),
    done: finished,
  });
}

pub async fn run(broker: &BrokerHandle, config: ServerConfig) -> Result<()> {
  let server = spawn(broker, config).await?;
  println!("Fenrir server listening on http://{}", server.address);
  shutdown_signal().await;
  server.shutdown().await;
  return Ok(());
}

fn adapter(broker: &BrokerHandle) -> AdapterHandle {
  let cwd = std::env::current_dir()
    .map(|path| path.display().to_string())
    .unwrap_or_default();
  return AdapterHandle {
    broker: broker.clone(),
    cwd,
    connections: ConnectionsHandle::spawn(broker.clone()),
  };
}

impl AdapterHandle {
  pub fn cwd(&self) -> &str {
    return &self.cwd;
  }

  pub async fn connect(&self) -> Result<TempClient> {
    return TempClient::connect(&self.broker, &self.cwd).await;
  }

  pub async fn create_connection(&self, cwd: String) -> Result<uuid::Uuid> {
    return self.connections.create(cwd).await;
  }

  pub fn connections(&self) -> ConnectionsHandle {
    return self.connections.clone();
  }

  pub async fn connection_sender(
    &self,
    client_id: uuid::Uuid,
  ) -> Result<Option<ClientSender>> {
    return self.connections.sender(client_id).await;
  }

  pub async fn take_events(
    &self,
    client_id: uuid::Uuid,
  ) -> Result<EventsReply> {
    return self.connections.take_events(client_id).await;
  }

  pub async fn disconnect_connection(
    &self,
    client_id: uuid::Uuid,
  ) -> Result<()> {
    return self.connections.disconnect(client_id).await;
  }

  pub async fn list_sessions(
    &self,
    limit: Option<u32>,
  ) -> Result<Vec<SessionSummary>> {
    let mut client = self.connect().await?;
    let result = client.list_sessions(limit).await;
    client.disconnect().await;
    return result;
  }

  async fn shutdown(&self) {
    let Ok(client) = self.connect().await else {
      return;
    };
    let _ = time::timeout(SHUTDOWN_TIMEOUT, client.shutdown()).await;
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
