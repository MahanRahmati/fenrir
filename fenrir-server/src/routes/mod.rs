mod commands;
mod connections;
mod health;
mod models;
mod personas;
mod request;
mod sessions;
mod stream;

use axum::Router;
use axum::routing::{delete, get, post, put};

use crate::adapter::AdapterHandle;

pub fn router() -> Router<AdapterHandle> {
  return Router::new()
    .merge(service_routes())
    .nest("/api/connections/{client_id}", connection_routes());
}

fn service_routes() -> Router<AdapterHandle> {
  return Router::new()
    .route("/api/health", get(health::health))
    .route("/api/defaults", get(connections::defaults))
    .route("/api/connections", post(connections::create))
    .route("/api/sessions", get(sessions::list_sessions))
    .route("/api/sessions/{session_id}", get(sessions::get_session))
    .route("/api/models", get(models::list_models))
    .route("/api/personas", get(personas::list_personas))
    .route("/api/commands", get(commands::list_commands));
}

fn connection_routes() -> Router<AdapterHandle> {
  return Router::new()
    .route("/", delete(connections::delete))
    .route("/events", get(connections::events))
    .route("/sessions", post(connections::create_session))
    .nest("/sessions/{session_id}", session_routes())
    .route("/prompts", post(connections::prompt))
    .route("/persona", post(connections::set_persona))
    .route("/model", post(connections::set_model));
}

fn session_routes() -> Router<AdapterHandle> {
  return Router::new()
    .route(
      "/",
      put(connections::subscribe).delete(connections::unsubscribe),
    )
    .route("/review", post(connections::review))
    .route("/compact", post(connections::compact))
    .route("/cancel", post(connections::cancel))
    .route("/approval", post(connections::approval))
    .route("/predict", post(connections::predict));
}
