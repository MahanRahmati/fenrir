use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use fenrir_core::engine::protocol::{ServerEvent, SessionSummary, SessionView};
use fenrir_core::provider_manager::Model;

use super::request::{self, await_event};
use crate::adapter::AdapterHandle;
use crate::api_request::ApiRequest;
use crate::api_response::{ApiError, ApiResponse, ApiResult};

#[derive(Debug, Serialize)]
pub struct SessionsData {
  pub sessions: Vec<SessionSummary>,
}

#[derive(Debug, Serialize)]
pub struct SessionData {
  pub session: SessionView,
}

#[derive(Debug, Serialize)]
pub struct PersonaData {
  pub persona: String,
}

#[derive(Debug, Serialize)]
pub struct ModelData {
  pub model: Model,
}

#[derive(Debug, Serialize)]
pub struct CancelData {
  pub accepted: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct ListSessionsQuery {
  pub limit: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CreateSessionRequest {
  pub persona: Option<String>,
  pub model: Option<Model>,
}

#[derive(Debug, Deserialize)]
pub struct PersonaRequest {
  pub persona: String,
}

#[derive(Debug, Deserialize)]
pub struct ModelRequest {
  pub model: Model,
}

impl ApiRequest for CreateSessionRequest {
  fn validate(&self) -> Result<(), ApiError> {
    if let Some(persona) = &self.persona
      && persona.trim().is_empty()
    {
      return Err(ApiError::new(
        StatusCode::BAD_REQUEST,
        "Persona is required",
      ));
    }
    if let Some(model) = &self.model {
      validate_model(model)?;
    }
    return Ok(());
  }
}

impl ApiRequest for PersonaRequest {
  fn validate(&self) -> Result<(), ApiError> {
    if self.persona.trim().is_empty() {
      return Err(ApiError::new(
        StatusCode::BAD_REQUEST,
        "Persona is required",
      ));
    }
    return Ok(());
  }
}

impl ApiRequest for ModelRequest {
  fn validate(&self) -> Result<(), ApiError> {
    return validate_model(&self.model);
  }
}

fn validate_model(model: &Model) -> Result<(), ApiError> {
  if model.provider.trim().is_empty() || model.name.trim().is_empty() {
    return Err(ApiError::new(
      StatusCode::BAD_REQUEST,
      "Model provider and name are required",
    ));
  }
  return Ok(());
}

pub async fn list_sessions(
  State(adapter): State<AdapterHandle>,
  Query(query): Query<ListSessionsQuery>,
) -> ApiResult<SessionsData> {
  let sessions = adapter.list_sessions(query.limit).await?;
  return Ok(ApiResponse::success(
    SessionsData { sessions },
    "Sessions listed",
  ));
}

pub async fn create_session(
  State(adapter): State<AdapterHandle>,
  Json(request): Json<CreateSessionRequest>,
) -> ApiResult<SessionData> {
  request.validate()?;

  let mut client = adapter.connect().await?;
  let result = async {
    let request_id = client
      .create_session(request.persona, request.model)
      .await?;
    return await_event(&mut client, request_id, |event| match event {
      ServerEvent::SessionCreated {
        request_id: id,
        session,
      } if id == request_id => Some(session),
      _ => None,
    })
    .await;
  }
  .await;
  let _ = client.disconnect().await;

  let session = result?;
  return Ok(ApiResponse::success(
    SessionData { session },
    "Session created",
  ));
}

pub async fn get_session(
  State(adapter): State<AdapterHandle>,
  Path(session_id): Path<Uuid>,
) -> ApiResult<SessionData> {
  let mut client = adapter.connect().await?;
  let result = request::load_session(&mut client, session_id).await;
  let _ = client.disconnect().await;

  let session = result?;
  return Ok(ApiResponse::success(
    SessionData { session },
    "Session loaded",
  ));
}

pub async fn set_persona(
  State(adapter): State<AdapterHandle>,
  Path(session_id): Path<Uuid>,
  Json(request): Json<PersonaRequest>,
) -> ApiResult<PersonaData> {
  request.validate()?;

  let mut client = adapter.connect().await?;
  let result = async {
    request::load_session(&mut client, session_id).await?;
    let request_id = client
      .set_persona_for(Some(session_id), request.persona)
      .await?;
    return await_event(&mut client, request_id, |event| match event {
      ServerEvent::PersonaUpdated {
        request_id: id,
        persona,
        ..
      } if id == request_id => Some(persona),
      _ => None,
    })
    .await;
  }
  .await;
  let _ = client.disconnect().await;

  let persona = result?;
  return Ok(ApiResponse::success(
    PersonaData { persona },
    "Persona updated",
  ));
}

pub async fn set_model(
  State(adapter): State<AdapterHandle>,
  Path(session_id): Path<Uuid>,
  Json(request): Json<ModelRequest>,
) -> ApiResult<ModelData> {
  request.validate()?;

  let mut client = adapter.connect().await?;
  let result = async {
    request::load_session(&mut client, session_id).await?;
    let request_id = client
      .set_model_for(Some(session_id), request.model)
      .await?;
    return await_event(&mut client, request_id, |event| match event {
      ServerEvent::ModelUpdated {
        request_id: id,
        model,
        ..
      } if id == request_id => Some(model),
      _ => None,
    })
    .await;
  }
  .await;
  let _ = client.disconnect().await;

  let model = result?;
  return Ok(ApiResponse::success(ModelData { model }, "Model updated"));
}

pub async fn cancel(
  State(adapter): State<AdapterHandle>,
  Path(session_id): Path<Uuid>,
) -> ApiResult<CancelData> {
  let mut client = adapter.connect().await?;
  let result: Result<(), ApiError> = async {
    request::load_session(&mut client, session_id).await?;
    client.cancel(None).await?;
    return Ok(());
  }
  .await;
  let _ = client.disconnect().await;

  result?;
  return Ok(ApiResponse::success(
    CancelData { accepted: true },
    "Turn cancellation requested",
  ));
}
