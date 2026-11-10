use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;

use fenrir_core::engine::protocol::ServerEvent;
use fenrir_core::provider_manager::Model;

use super::request::await_event;
use crate::adapter::AdapterHandle;
use crate::api_response::{ApiError, ApiResponse, ApiResult};

#[derive(Debug, Serialize)]
pub struct ModelsData {
  pub models: Vec<Model>,
  pub current: Option<Model>,
}

#[derive(Debug, Serialize)]
pub struct DefaultModelData {
  pub model: Model,
}

pub async fn list_models(
  State(adapter): State<AdapterHandle>,
) -> ApiResult<ModelsData> {
  let mut client = adapter.connect().await?;
  let result = async {
    let request_id = client.list_models(None).await?;
    return await_event(&mut client, request_id, |event| match event {
      ServerEvent::ModelsListed {
        request_id: id,
        models,
        current,
      } if id == request_id => Some((models, current)),
      _ => None,
    })
    .await;
  }
  .await;
  let _ = client.disconnect().await;

  let (models, current) = result?;
  return Ok(ApiResponse::success(
    ModelsData { models, current },
    "Models listed",
  ));
}

pub async fn default_model(
  State(adapter): State<AdapterHandle>,
) -> ApiResult<DefaultModelData> {
  let mut client = adapter.connect().await?;
  let result = async {
    let request_id = client.list_models(None).await?;
    return await_event(&mut client, request_id, |event| match event {
      ServerEvent::ModelsListed {
        request_id: id,
        current,
        ..
      } if id == request_id => Some(current),
      _ => None,
    })
    .await;
  }
  .await;
  let _ = client.disconnect().await;

  let Some(model) = result? else {
    return Err(ApiError::new(
      StatusCode::NOT_FOUND,
      "No default model configured",
    ));
  };
  return Ok(ApiResponse::success(
    DefaultModelData { model },
    "Default model",
  ));
}
