use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
  pub role: String,
  pub content: String,
  pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
  pub id: i64,
  pub created_at: i64,
  pub last_interaction: i64,
  pub messages: Vec<Message>,
}

impl Session {
  pub fn new() -> Self {
    let now = Utc::now().timestamp();
    return Self {
      id: now,
      created_at: now,
      last_interaction: now,
      messages: Vec::new(),
    };
  }

  pub fn add_system_message(&mut self, content: String) {
    self.add_message("system", content);
  }

  pub fn add_user_message(&mut self, content: String) {
    self.add_message("user", content);
  }

  pub fn add_assistant_message(&mut self, content: String) {
    self.add_message("assistant", content);
  }

  fn add_message(&mut self, role: &str, content: String) {
    let now = Utc::now().timestamp();
    let message = Message {
      role: role.to_string(),
      content,
      timestamp: now,
    };
    self.last_interaction = now;
    self.messages.push(message);
  }
}
