mod approval;
mod command;
mod commands;
mod health;
mod models;
mod personas;
mod prompt;
mod request;
mod sessions;
mod stream;

use axum::Router;
use axum::routing::{get, post};

use crate::adapter::AdapterHandle;

pub fn router() -> Router<AdapterHandle> {
  return Router::new()
    .route("/api/health", get(health::health))
    .route(
      "/api/sessions",
      get(sessions::list_sessions).post(sessions::create_session),
    )
    .route("/api/sessions/{session_id}", get(sessions::get_session))
    .route(
      "/api/sessions/{session_id}/persona",
      post(sessions::set_persona),
    )
    .route(
      "/api/sessions/{session_id}/model",
      post(sessions::set_model),
    )
    .route("/api/sessions/{session_id}/prompt", post(prompt::prompt))
    .route("/api/sessions/{session_id}/cancel", post(sessions::cancel))
    .route(
      "/api/sessions/{session_id}/approval",
      post(approval::approve),
    )
    .route("/api/sessions/{session_id}/command", post(command::command))
    .route("/api/sessions/{session_id}/compact", post(command::compact))
    .route("/api/models", get(models::list_models))
    .route("/api/models/default", get(models::default_model))
    .route("/api/personas", get(personas::list_personas))
    .route("/api/commands", get(commands::list_commands));
}
