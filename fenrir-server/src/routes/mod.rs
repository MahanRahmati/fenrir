pub mod health;
pub mod sessions;

use axum::Router;
use axum::routing::get;

use crate::adapter::AdapterHandle;

pub fn router() -> Router<AdapterHandle> {
  return Router::new()
    .route("/api/health", get(health::health))
    .route("/api/sessions", get(sessions::list_sessions));
}
