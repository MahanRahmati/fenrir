use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::request;
use crate::adapter::AdapterHandle;
use crate::api_request::ApiRequest;
use crate::api_response::{ApiError, ApiResponse, ApiResult};

#[derive(Debug, Deserialize)]
pub struct ApprovalRequest {
  pub turn_id: Uuid,
  pub tool_call_id: String,
  pub approved: bool,
}

#[derive(Debug, Serialize)]
pub struct ApprovalData {
  pub accepted: bool,
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

pub async fn approve(
  State(adapter): State<AdapterHandle>,
  Path(session_id): Path<Uuid>,
  Json(request): Json<ApprovalRequest>,
) -> ApiResult<ApprovalData> {
  request.validate()?;

  let mut client = adapter.connect().await?;
  let result: Result<(), ApiError> = async {
    request::load_session(&mut client, session_id).await?;
    client
      .approve_tool(
        session_id,
        request.turn_id,
        request.tool_call_id,
        request.approved,
      )
      .await?;
    return Ok(());
  }
  .await;
  let _ = client.disconnect().await;

  result?;
  return Ok(ApiResponse::success(
    ApprovalData { accepted: true },
    "Tool approval submitted",
  ));
}
