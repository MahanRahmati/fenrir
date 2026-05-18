use chrono::{DateTime, Utc};

pub struct Session {
  pub id: i64,
  pub created_at: i64,
  pub last_interaction: i64,
}

impl Session {
  pub fn new() -> Self {
    let now = Utc::now().timestamp();
    return Self {
      id: now,
      created_at: now,
      last_interaction: now,
    };
  }
}
