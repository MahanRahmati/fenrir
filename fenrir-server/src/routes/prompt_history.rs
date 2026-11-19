use axum::extract::State;

use fenrir_core::client::api::PromptHistoryData;

use crate::adapter::AdapterHandle;
use crate::api_response::{ApiResult, success};

pub async fn list_prompt_history(
  State(adapter): State<AdapterHandle>,
) -> ApiResult<PromptHistoryData> {
  let prompts = adapter.list_prompt_history().await?;
  return Ok(success(
    PromptHistoryData { prompts },
    "Prompt history listed",
  ));
}
