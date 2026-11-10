use axum::extract::State;
use serde::Serialize;

use fenrir_core::engine::protocol::{PersonaInfo, ServerEvent};

use super::request::await_event;
use crate::adapter::AdapterHandle;
use crate::api_response::{ApiResponse, ApiResult};

#[derive(Debug, Serialize)]
pub struct PersonasData {
  pub personas: Vec<PersonaInfo>,
  pub current: Option<String>,
}

pub async fn list_personas(
  State(adapter): State<AdapterHandle>,
) -> ApiResult<PersonasData> {
  let mut client = adapter.connect().await?;
  let result = async {
    let request_id = client.list_personas().await?;
    return await_event(&mut client, request_id, |event| match event {
      ServerEvent::PersonasListed {
        request_id: id,
        personas,
        current,
      } if id == request_id => Some((personas, current)),
      _ => None,
    })
    .await;
  }
  .await;
  let _ = client.disconnect().await;

  let (personas, current) = result?;
  return Ok(ApiResponse::success(
    PersonasData { personas, current },
    "Personas listed",
  ));
}
