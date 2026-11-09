use std::time::Duration;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc::Receiver;

use fenrir_core::config::ServerConfig;
use fenrir_core::engine::broker::BrokerHandle;
use fenrir_core::engine::protocol::{ClientRequest, ServerEvent};
use fenrir_core::vlog;

use crate::routes;

pub async fn run(broker: &BrokerHandle, config: ServerConfig) -> Result<()> {
  let cwd = std::env::current_dir()
    .map(|path| path.display().to_string())
    .unwrap_or_default();
  let client = broker.connect(&cwd).await?;
  let sender = client.sender();
  tokio::spawn(drain_events(client.events));

  let app = routes::router();
  let address = format!("{}:{}", config.host, config.port);
  let listener = TcpListener::bind(&address)
    .await
    .context(format!("Failed to bind server to '{address}'"))?;
  let local_address = listener
    .local_addr()
    .context("Failed to read server address")?;
  println!("Fenrir server listening on http://{local_address}");

  axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("Server failed")?;

  let _ = tokio::time::timeout(
    Duration::from_secs(2),
    sender.send(ClientRequest::Shutdown),
  )
  .await;
  return Ok(());
}

async fn drain_events(mut events: Receiver<ServerEvent>) {
  while let Some(event) = events.recv().await {
    vlog!("[server] event: {event:?}");
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
