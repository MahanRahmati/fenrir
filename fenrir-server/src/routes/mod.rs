pub mod health;

use axum::Router;
use axum::routing::get;

pub fn router() -> Router {
  return Router::new().route("/api/health", get(health::health));
}
