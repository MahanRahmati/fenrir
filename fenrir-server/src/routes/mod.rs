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

use fenrir_core::client::urls;

use crate::adapter::AdapterHandle;

pub fn router() -> Router<AdapterHandle> {
  return Router::new()
    .route(urls::HEALTH, get(health::health))
    .route(urls::DEFAULTS, get(connections::defaults))
    .route(urls::CONNECTIONS, post(connections::create))
    .route(urls::SESSIONS, get(sessions::list_sessions))
    .route(urls::SESSION, get(sessions::get_session))
    .route(urls::MODELS, get(models::list_models))
    .route(urls::PERSONAS, get(personas::list_personas))
    .route(urls::COMMANDS, get(commands::list_commands))
    .route(urls::CONNECTION, delete(connections::delete))
    .route(urls::CONNECTION_EVENTS, get(connections::events))
    .route(urls::CONNECTION_SESSIONS, post(connections::create_session))
    .route(
      urls::CONNECTION_SESSION,
      put(connections::subscribe).delete(connections::unsubscribe),
    )
    .route(urls::CONNECTION_PROMPTS, post(connections::prompt))
    .route(urls::CONNECTION_PERSONA, post(connections::set_persona))
    .route(urls::CONNECTION_MODEL, post(connections::set_model))
    .route(urls::CONNECTION_SESSION_REVIEW, post(connections::review))
    .route(urls::CONNECTION_SESSION_COMPACT, post(connections::compact))
    .route(urls::CONNECTION_SESSION_CANCEL, post(connections::cancel))
    .route(
      urls::CONNECTION_SESSION_APPROVAL,
      post(connections::approval),
    )
    .route(urls::CONNECTION_SESSION_PREDICT, post(connections::predict));
}
