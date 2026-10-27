use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolApprovalRequest {
  pub tool_call_id: String,
  pub session_id: Uuid,
  pub tool_name: String,
  pub message: String,
  pub created_at: i64,
}

impl ToolApprovalRequest {
  pub fn new(
    tool_call_id: String,
    session_id: Uuid,
    tool_name: String,
    message: String,
  ) -> Self {
    let now = Utc::now().timestamp();
    return Self {
      tool_call_id,
      session_id,
      tool_name,
      message,
      created_at: now,
    };
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolApprovalStatus {
  Pending,
  Approved,
  Denied,
}

impl ToolApprovalStatus {
  pub fn as_str(&self) -> &'static str {
    return match self {
      ToolApprovalStatus::Pending => "pending",
      ToolApprovalStatus::Approved => "approved",
      ToolApprovalStatus::Denied => "denied",
    };
  }

  pub fn parse(value: &str) -> Option<Self> {
    return match value {
      "pending" => Some(ToolApprovalStatus::Pending),
      "approved" => Some(ToolApprovalStatus::Approved),
      "denied" => Some(ToolApprovalStatus::Denied),
      _ => None,
    };
  }
}
