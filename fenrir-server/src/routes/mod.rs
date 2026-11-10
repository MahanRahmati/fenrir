mod health;
mod prompt;
mod sessions;

use axum::Router;
use axum::routing::{get, post};

use crate::adapter::AdapterHandle;

pub fn router() -> Router<AdapterHandle> {
  return Router::new()
    .route("/api/health", get(health::health))
    .route("/api/sessions", get(sessions::list_sessions))
    .route("/api/sessions/{session_id}/prompt", post(prompt::prompt));
}
