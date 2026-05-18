mod session;

use std::path::PathBuf;

use crate::session_manager::session::Session;

pub struct SessionManager {
  sessions_path: PathBuf,
  sessions: Vec<Session>,
}

impl SessionManager {
  pub fn new(path: String) -> Self {
    let sessions_path = PathBuf::from(path);
    return Self {
      sessions_path,
      sessions: Vec::new(),
    };
  }

  pub fn get_sessions(&self) -> &Vec<Session> {
    return &self.sessions;
  }

  pub fn get_session_by_id(&self, id: i64) -> Option<&Session> {
    return self.sessions.iter().find(|session| session.id == id);
  }
}
