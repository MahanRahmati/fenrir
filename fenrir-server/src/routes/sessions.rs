use axum::extract::State;
use serde::Serialize;

use fenrir_core::engine::protocol::SessionSummary;

use crate::adapter::AdapterHandle;
use crate::api_response::{ApiResponse, ApiResult};

#[derive(Debug, Serialize)]
pub struct SessionsData {
  pub sessions: Vec<SessionSummary>,
}

pub async fn list_sessions(
  State(adapter): State<AdapterHandle>,
) -> ApiResult<SessionsData> {
  let sessions = adapter.list_sessions().await?;
  return Ok(ApiResponse::success(
    SessionsData { sessions },
    "Sessions listed",
  ));
}
