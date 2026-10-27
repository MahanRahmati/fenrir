use tokio::sync::broadcast::{self, Receiver, Sender};

use crate::session_manager::ToolApprovalStatus;

const CAPACITY: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDecision {
  pub tool_call_id: String,
  pub status: ToolApprovalStatus,
}

#[derive(Clone)]
pub struct ApprovalChannel {
  sender: Sender<ApprovalDecision>,
}

impl ApprovalChannel {
  pub fn new() -> Self {
    let (sender, _) = broadcast::channel(CAPACITY);
    return Self { sender };
  }

  pub fn subscribe(&self) -> Receiver<ApprovalDecision> {
    return self.sender.subscribe();
  }

  pub fn decide(&self, tool_call_id: &str, status: ToolApprovalStatus) {
    self
      .sender
      .send(ApprovalDecision {
        tool_call_id: tool_call_id.to_string(),
        status,
      })
      .ok();
  }
}

impl Default for ApprovalChannel {
  fn default() -> Self {
    return Self::new();
  }
}
