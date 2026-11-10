use axum::extract::{Path, State};
use axum::response::IntoResponse;
use uuid::Uuid;

use super::request;
use super::stream::{self, StreamEnd};
use crate::adapter::AdapterHandle;
use crate::api_response::ApiStreamResult;

pub async fn events(
  State(adapter): State<AdapterHandle>,
  Path(session_id): Path<Uuid>,
) -> ApiStreamResult<impl IntoResponse> {
  let mut client = adapter.connect().await?;
  if let Err(error) = request::load_session(&mut client, session_id).await {
    let _ = client.disconnect().await;
    return Err(error);
  }

  return Ok(stream::session_stream(client, session_id, StreamEnd::Live));
}
