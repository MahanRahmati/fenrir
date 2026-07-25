use tokio::signal::unix::{SignalKind, signal};
use tokio_util::sync::CancellationToken;

use crate::vlog;

/// Manages graceful shutdown for the application.
///
/// Handles OS signals (SIGINT, SIGTERM) and provides cancellation tokens
/// for tasks that need to respond to shut down.
pub struct Shutdown {
  token: CancellationToken,
}

impl Shutdown {
  /// Create a new `Shutdown` and start listening for signals.
  pub fn new() -> Self {
    let token = CancellationToken::new();
    let cloned_token = token.clone();

    tokio::spawn(async move {
      let mut sigint = signal(SignalKind::interrupt())
        .expect("Failed to register SIGINT signal");
      let mut sigterm = signal(SignalKind::terminate())
        .expect("Failed to register SIGTERM signal");

      tokio::select! {
        _ = sigint.recv() => {
          vlog!("Received SIGINT, shutting down...");
          cloned_token.cancel();
        }
        _ = sigterm.recv() => {
          vlog!("Received SIGTERM, shutting down...");
          cloned_token.cancel();
        }
      }
    });

    return Self { token };
  }

  /// Returns `true` if shutdown has been requested.
  pub fn is_cancelled(&self) -> bool {
    self.token.is_cancelled()
  }
}
