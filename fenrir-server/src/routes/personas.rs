use axum::extract::State;

use fenrir_core::client::api::PersonasData;
use fenrir_core::engine::protocol::ServerEvent;

use super::request::await_event;
use crate::adapter::AdapterHandle;
use crate::api_response::{ApiResult, success};

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
        current: _,
      } if id == request_id => Some(personas),
      _ => None,
    })
    .await;
  }
  .await;
  let _ = client.disconnect().await;

  let personas = result?;
  return Ok(success(PersonasData { personas }, "Personas listed"));
}
