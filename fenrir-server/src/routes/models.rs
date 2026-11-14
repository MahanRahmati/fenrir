use axum::extract::State;

use fenrir_core::client::api::ModelsData;

use crate::adapter::AdapterHandle;
use crate::api_response::{ApiResult, success};

pub async fn list_models(
  State(adapter): State<AdapterHandle>,
) -> ApiResult<ModelsData> {
  let mut client = adapter.connect().await?;
  let result = client.list_models().await;
  client.disconnect().await;

  let (models, _) = result?;
  return Ok(success(ModelsData { models }, "Models listed"));
}
