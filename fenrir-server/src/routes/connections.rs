use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use uuid::Uuid;

use fenrir_core::client::api::{
  ApprovalRequest, CancelRequest, CompactRequest, ConnectionData,
  CreateConnectionRequest, CreateSessionRequest, DefaultsData, ModelRequest,
  PersonaRequest, PromptRequest, ReviewRequest, SubscribeRequest,
};
use fenrir_core::command::Command;
use fenrir_core::engine::protocol::{ClientRequest, PromptMode, ServerEvent};
use fenrir_core::provider_manager::Model;

use super::request::await_event;
use super::stream;
use crate::adapter::AdapterHandle;
use crate::api_request::ApiRequest;
use crate::api_response::{
  ApiAccepted, ApiAction, ApiError, ApiResult, ApiStreamResult, success,
};
use crate::connections::EventsReply;

impl ApiRequest for CreateConnectionRequest {
  fn validate(&self) -> Result<(), ApiError> {
    if self.cwd.as_deref().is_some_and(|cwd| cwd.trim().is_empty()) {
      return Err(ApiError::new(
        StatusCode::BAD_REQUEST,
        "Working directory is required",
      ));
    }
    return Ok(());
  }
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

impl ApiRequest for PersonaRequest {
  fn validate(&self) -> Result<(), ApiError> {
    if self.name.trim().is_empty() {
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

impl ApiRequest for ApprovalRequest {
  fn validate(&self) -> Result<(), ApiError> {
    if self.tool_call_id.trim().is_empty() {
      return Err(ApiError::new(
        StatusCode::BAD_REQUEST,
        "Tool call ID is required",
      ));
    }
    return Ok(());
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

pub async fn create(
  State(adapter): State<AdapterHandle>,
  payload: Option<Json<CreateConnectionRequest>>,
) -> ApiResult<ConnectionData> {
  let request = payload.map(|Json(request)| request).unwrap_or_default();
  request.validate()?;
  let cwd = request.cwd.unwrap_or_else(|| adapter.cwd().to_string());
  let client_id = adapter.create_connection(cwd).await?;
  return Ok(success(ConnectionData { client_id }, "Connection created"));
}

pub async fn events(
  State(adapter): State<AdapterHandle>,
  Path(client_id): Path<Uuid>,
) -> ApiStreamResult<impl IntoResponse> {
  return match adapter.take_events(client_id).await? {
    EventsReply::Taken(events) => Ok(stream::connection_stream(
      events,
      adapter.connections(),
      client_id,
    )),
    EventsReply::Missing => {
      Err(ApiError::new(StatusCode::NOT_FOUND, "Connection not found"))
    }
    EventsReply::AlreadyOpen => Err(ApiError::new(
      StatusCode::CONFLICT,
      "Connection stream is already open",
    )),
  };
}

pub async fn delete(
  State(adapter): State<AdapterHandle>,
  Path(client_id): Path<Uuid>,
) -> ApiAction<()> {
  adapter.disconnect_connection(client_id).await?;
  return Ok(ApiAccepted::accepted("Connection closed"));
}

pub async fn subscribe(
  State(adapter): State<AdapterHandle>,
  Path((client_id, session_id)): Path<(Uuid, Uuid)>,
  Json(request): Json<SubscribeRequest>,
) -> ApiAction<()> {
  let sender = sender(&adapter, client_id).await?;
  sender
    .send(ClientRequest::LoadSession {
      request_id: request.request_id,
      session_id,
    })
    .await?;
  return Ok(ApiAccepted::accepted("Session subscription requested"));
}

pub async fn unsubscribe(
  State(adapter): State<AdapterHandle>,
  Path((client_id, session_id)): Path<(Uuid, Uuid)>,
) -> ApiAction<()> {
  let sender = sender(&adapter, client_id).await?;
  sender
    .send(ClientRequest::Unsubscribe { session_id })
    .await?;
  return Ok(ApiAccepted::accepted("Session unsubscribed"));
}

pub async fn create_session(
  State(adapter): State<AdapterHandle>,
  Path(client_id): Path<Uuid>,
  Json(request): Json<CreateSessionRequest>,
) -> ApiAction<()> {
  request.validate()?;
  let sender = sender(&adapter, client_id).await?;
  sender
    .send(ClientRequest::CreateSession {
      request_id: request.request_id,
      persona: request.persona,
      model: request.model,
    })
    .await?;
  return Ok(ApiAccepted::accepted("Session creation requested"));
}

pub async fn prompt(
  State(adapter): State<AdapterHandle>,
  Path(client_id): Path<Uuid>,
  Json(request): Json<PromptRequest>,
) -> ApiAction<()> {
  request.validate()?;
  let sender = sender(&adapter, client_id).await?;
  sender
    .send(ClientRequest::SendPrompt {
      request_id: request.request_id,
      session_id: request.session_id,
      mode: request.mode.unwrap_or(PromptMode::Normal),
      text: Some(request.text),
    })
    .await?;
  return Ok(ApiAccepted::accepted("Prompt submitted"));
}

pub async fn set_persona(
  State(adapter): State<AdapterHandle>,
  Path(client_id): Path<Uuid>,
  Json(request): Json<PersonaRequest>,
) -> ApiAction<()> {
  request.validate()?;
  let sender = sender(&adapter, client_id).await?;
  sender
    .send(ClientRequest::SetPersona {
      request_id: request.request_id,
      session_id: request.session_id,
      name: request.name,
    })
    .await?;
  return Ok(ApiAccepted::accepted("Persona change requested"));
}

pub async fn set_model(
  State(adapter): State<AdapterHandle>,
  Path(client_id): Path<Uuid>,
  Json(request): Json<ModelRequest>,
) -> ApiAction<()> {
  request.validate()?;
  let sender = sender(&adapter, client_id).await?;
  sender
    .send(ClientRequest::SetModel {
      request_id: request.request_id,
      session_id: request.session_id,
      model: request.model,
    })
    .await?;
  return Ok(ApiAccepted::accepted("Model change requested"));
}

pub async fn review(
  State(adapter): State<AdapterHandle>,
  Path((client_id, session_id)): Path<(Uuid, Uuid)>,
  Json(request): Json<ReviewRequest>,
) -> ApiAction<()> {
  let sender = sender(&adapter, client_id).await?;
  sender
    .send(ClientRequest::RunCommand {
      request_id: request.request_id,
      session_id: Some(session_id),
      command: Command::Review,
    })
    .await?;
  return Ok(ApiAccepted::accepted("Review requested"));
}

pub async fn compact(
  State(adapter): State<AdapterHandle>,
  Path((client_id, session_id)): Path<(Uuid, Uuid)>,
  Json(request): Json<CompactRequest>,
) -> ApiAction<()> {
  let sender = sender(&adapter, client_id).await?;
  sender
    .send(ClientRequest::Compact {
      request_id: request.request_id,
      session_id,
    })
    .await?;
  return Ok(ApiAccepted::accepted("Compaction requested"));
}

pub async fn cancel(
  State(adapter): State<AdapterHandle>,
  Path((client_id, session_id)): Path<(Uuid, Uuid)>,
  payload: Option<Json<CancelRequest>>,
) -> ApiAction<()> {
  let request = payload.map(|Json(request)| request).unwrap_or_default();
  let sender = sender(&adapter, client_id).await?;
  sender
    .send(ClientRequest::Cancel {
      session_id,
      turn_id: request.turn_id,
    })
    .await?;
  return Ok(ApiAccepted::accepted("Cancellation requested"));
}

pub async fn approval(
  State(adapter): State<AdapterHandle>,
  Path((client_id, session_id)): Path<(Uuid, Uuid)>,
  Json(request): Json<ApprovalRequest>,
) -> ApiAction<()> {
  request.validate()?;
  let sender = sender(&adapter, client_id).await?;
  sender
    .send(ClientRequest::ApproveTool {
      session_id,
      turn_id: request.turn_id,
      tool_call_id: request.tool_call_id,
      approved: request.approved,
    })
    .await?;
  return Ok(ApiAccepted::accepted("Tool approval submitted"));
}

pub async fn predict(
  State(adapter): State<AdapterHandle>,
  Path((client_id, session_id)): Path<(Uuid, Uuid)>,
) -> ApiAction<()> {
  let sender = sender(&adapter, client_id).await?;
  sender.send(ClientRequest::Predict { session_id }).await?;
  return Ok(ApiAccepted::accepted("Prediction requested"));
}

pub async fn defaults(
  State(adapter): State<AdapterHandle>,
) -> ApiResult<DefaultsData> {
  let mut client = adapter.connect().await?;
  let result = async {
    let request_id = client.list_personas().await?;
    let persona = await_event(&mut client, request_id, |event| match event {
      ServerEvent::PersonasListed {
        request_id: id,
        current,
        ..
      } if id == request_id => Some(current),
      _ => None,
    })
    .await?;
    let request_id = client.list_models(None).await?;
    let model = await_event(&mut client, request_id, |event| match event {
      ServerEvent::ModelsListed {
        request_id: id,
        current,
        ..
      } if id == request_id => Some(current),
      _ => None,
    })
    .await?;
    return Ok::<_, ApiError>((persona, model));
  }
  .await;
  let _ = client.disconnect().await;

  let (persona, model) = result?;
  let Some(persona) = persona else {
    return Err(ApiError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      "No default persona configured",
    ));
  };
  return Ok(success(DefaultsData { persona, model }, "Defaults loaded"));
}

async fn sender(
  adapter: &AdapterHandle,
  client_id: Uuid,
) -> Result<fenrir_core::engine::broker::ClientSender, ApiError> {
  let sender = adapter.connection_sender(client_id).await?;
  return match sender {
    Some(sender) => Ok(sender),
    None => Err(ApiError::new(StatusCode::NOT_FOUND, "Connection not found")),
  };
}
