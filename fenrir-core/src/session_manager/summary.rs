use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
  pub id: Uuid,
  pub session_id: Uuid,
  pub content: String,
  pub covers_message_id: Uuid,
  pub created_at: i64,
}
