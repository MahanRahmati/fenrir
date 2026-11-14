use axum::extract::State;

use fenrir_core::client::api::PersonasData;

use crate::adapter::AdapterHandle;
use crate::api_response::{ApiResult, success};

pub async fn list_personas(
  State(adapter): State<AdapterHandle>,
) -> ApiResult<PersonasData> {
  let mut client = adapter.connect().await?;
  let result = client.list_personas().await;
  client.disconnect().await;

  let (personas, _) = result?;
  return Ok(success(PersonasData { personas }, "Personas listed"));
}
