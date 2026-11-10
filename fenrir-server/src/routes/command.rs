use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use uuid::Uuid;

use fenrir_core::command::Command;

use super::request;
use super::stream::{self, StreamEnd};
use crate::adapter::AdapterHandle;
use crate::api_request::ApiRequest;
use crate::api_response::{ApiError, ApiStreamResult};

#[derive(Debug, Deserialize)]
pub struct CommandRequest {
  pub command: String,
}

impl ApiRequest for CommandRequest {
  fn validate(&self) -> Result<(), ApiError> {
    if self.command.trim().is_empty() {
      return Err(ApiError::new(
        StatusCode::BAD_REQUEST,
        "Command is required",
      ));
    }
    return Ok(());
  }
}

pub async fn command(
  State(adapter): State<AdapterHandle>,
  Path(session_id): Path<Uuid>,
  Json(request): Json<CommandRequest>,
) -> ApiStreamResult<impl IntoResponse> {
  request.validate()?;

  let command = match Command::parse(&request.command) {
    Ok(Some(command)) => command,
    Ok(None) => {
      return Err(ApiError::new(
        StatusCode::BAD_REQUEST,
        "Command must start with '/'",
      ));
    }
    Err(error) => {
      return Err(ApiError::new(StatusCode::BAD_REQUEST, error.to_string()));
    }
  };

  return run(adapter, session_id, command).await;
}

pub async fn compact(
  State(adapter): State<AdapterHandle>,
  Path(session_id): Path<Uuid>,
) -> ApiStreamResult<impl IntoResponse> {
  let mut client = adapter.connect().await?;
  if let Err(error) = request::load_session(&mut client, session_id).await {
    let _ = client.disconnect().await;
    return Err(error);
  }

  let client_id = client.client_id();
  let request_id = match client.compact().await {
    Ok(request_id) => request_id,
    Err(error) => {
      let _ = client.disconnect().await;
      return Err(error.into());
    }
  };
  let end = StreamEnd::Turn {
    request_id,
    client_id,
  };
  return Ok(stream::session_stream(client, session_id, end));
}

async fn run(
  adapter: AdapterHandle,
  session_id: Uuid,
  command: Command,
) -> ApiStreamResult<impl IntoResponse> {
  let mut client = adapter.connect().await?;
  if let Err(error) = request::load_session(&mut client, session_id).await {
    let _ = client.disconnect().await;
    return Err(error);
  }

  let client_id = client.client_id();
  let request_id = match client.run_command(command).await {
    Ok(request_id) => request_id,
    Err(error) => {
      let _ = client.disconnect().await;
      return Err(error.into());
    }
  };
  let end = if command.starts_turn() {
    StreamEnd::Turn {
      request_id,
      client_id,
    }
  } else {
    StreamEnd::Request { request_id }
  };
  return Ok(stream::session_stream(client, session_id, end));
}
