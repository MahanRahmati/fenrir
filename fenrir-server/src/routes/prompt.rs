use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use uuid::Uuid;

use super::request;
use super::stream::{self, StreamEnd};
use crate::adapter::AdapterHandle;
use crate::api_request::ApiRequest;
use crate::api_response::{ApiError, ApiStreamResult};

#[derive(Debug, Deserialize)]
pub struct PromptRequest {
  pub text: String,
}

impl ApiRequest for PromptRequest {
  fn validate(&self) -> Result<(), ApiError> {
    if self.text.trim().is_empty() {
      return Err(ApiError::new(
        StatusCode::BAD_REQUEST,
        "Prompt text is required",
      ));
    }
    return Ok(());
  }
}

pub async fn prompt(
  State(adapter): State<AdapterHandle>,
  Path(session_id): Path<Uuid>,
  Json(request): Json<PromptRequest>,
) -> ApiStreamResult<impl IntoResponse> {
  request.validate()?;

  let mut client = adapter.connect().await?;
  if let Err(error) = request::load_session(&mut client, session_id).await {
    let _ = client.disconnect().await;
    return Err(error);
  }

  let request_id = match client.send_prompt(request.text).await {
    Ok(request_id) => request_id,
    Err(error) => {
      let _ = client.disconnect().await;
      return Err(error.into());
    }
  };
  let end = StreamEnd::Session { request_id };
  return Ok(stream::session_stream(client, session_id, end));
}
