mod session;

use std::path::PathBuf;

use crate::database::DB;
use crate::session_manager::session::Session;

pub struct SessionManager {
  sessions_path: PathBuf,
  sessions: Vec<Session>,
  current_session: Option<i64>,
  db: DB,
}

impl SessionManager {
  pub fn new(path: String, db: DB) -> Self {
    let sessions_path = PathBuf::from(path);
    return Self {
      sessions_path,
      sessions: Vec::new(),
      current_session: None,
      db,
    };
  }

  pub fn create_session(&mut self) -> i64 {
    let session = Session::new();
    let id = session.id;
    self.sessions.push(session);
    self.current_session = Some(id);
    return id;
  }

  pub fn add_user_message_to_current_session(&mut self, content: &str) {
    if let Some(session) = self
      .current_session
      .and_then(|id| self.sessions.iter_mut().find(|s| s.id == id))
    {
      session.add_user_message(content.to_string());
    }
  }

  pub fn get_sessions(&self) -> &Vec<Session> {
    return &self.sessions;
  }

  pub fn get_session_by_id(&self, id: i64) -> Option<&Session> {
    return self.sessions.iter().find(|session| session.id == id);
  }

  pub fn get_current_session(&self) -> Option<&Session> {
    return self
      .current_session
      .and_then(|id| self.get_session_by_id(id));
  }
}
