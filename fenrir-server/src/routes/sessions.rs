use axum::extract::{Path, Query, State};
use serde::Deserialize;
use uuid::Uuid;

use fenrir_core::client::api::{SessionData, SessionsData};

use crate::adapter::AdapterHandle;
use crate::api_response::{ApiResult, success};

#[derive(Debug, Default, Deserialize)]
pub struct ListSessionsQuery {
  pub limit: Option<u32>,
}

pub async fn list_sessions(
  State(adapter): State<AdapterHandle>,
  Query(query): Query<ListSessionsQuery>,
) -> ApiResult<SessionsData> {
  let sessions = adapter.list_sessions(query.limit).await?;
  return Ok(success(SessionsData { sessions }, "Sessions listed"));
}

pub async fn get_session(
  State(adapter): State<AdapterHandle>,
  Path(session_id): Path<Uuid>,
) -> ApiResult<SessionData> {
  let mut client = adapter.connect().await?;
  let result = client.load_session(session_id).await;
  client.disconnect().await;

  let session = result?;
  return Ok(success(SessionData { session }, "Session loaded"));
}
