use axum::extract::State;

use fenrir_core::client::api::ModelsData;
use fenrir_core::engine::protocol::ServerEvent;

use super::request::await_event;
use crate::adapter::AdapterHandle;
use crate::api_response::{ApiResult, success};

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
        current: _,
      } if id == request_id => Some(models),
      _ => None,
    })
    .await;
  }
  .await;
  let _ = client.disconnect().await;

  let models = result?;
  return Ok(success(ModelsData { models }, "Models listed"));
}
