mod errors;
mod queries;
mod session;

pub use crate::session_manager::session::Message;

use crate::database::DB;
use crate::session_manager::errors::{
  SessionManagerError, SessionManagerResult,
};
use crate::session_manager::queries::*;
use crate::session_manager::session::Session;

pub struct SessionManager {
  sessions: Vec<Session>,
  current_session: Option<i64>,
  db: DB,
}

impl SessionManager {
  pub fn new(db: DB) -> Self {
    return Self {
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

  pub async fn add_system_message_to_current_session(
    &mut self,
    content: &str,
  ) -> SessionManagerResult<()> {
    self
      .add_message_to_current_session(|session| {
        session.add_system_message(content);
      })
      .await?;
    return Ok(());
  }

  pub async fn add_user_message_to_current_session(
    &mut self,
    content: &str,
  ) -> SessionManagerResult<()> {
    self
      .add_message_to_current_session(|session| {
        session.add_user_message(content);
      })
      .await?;
    return Ok(());
  }

  pub async fn add_assistant_message_to_current_session(
    &mut self,
    content: &str,
  ) -> SessionManagerResult<()> {
    self
      .add_message_to_current_session(|session| {
        session.add_assistant_message(content);
      })
      .await?;
    return Ok(());
  }

  pub async fn add_tool_result_to_current_session(
    &mut self,
    tool_name: &str,
    result: &str,
  ) -> SessionManagerResult<()> {
    self
      .add_message_to_current_session(|session| {
        session.add_tool_message(&format!(
          "Tool '{}' result: {}",
          tool_name, result
        ));
      })
      .await?;
    return Ok(());
  }

  async fn add_message_to_current_session<F>(
    &mut self,
    f: F,
  ) -> SessionManagerResult<()>
  where
    F: FnOnce(&mut Session),
  {
    let session_id = match self.current_session {
      Some(id) => id,
      None => return Err(SessionManagerError::NoCurrentSession),
    };
    if let Some(session) = self.sessions.iter_mut().find(|s| s.id == session_id)
    {
      f(session);
      for msg in &session.messages {
        match self
          .db
          .execute(
            INSERT_MESSAGE,
            (session.id, &*msg.role, &*msg.content, msg.timestamp),
          )
          .await
        {
          Ok(_) => {}
          Err(e) => {
            return Err(SessionManagerError::DatabaseError(e.to_string()));
          }
        }
      }
    }
    return Ok(());
  }

  pub async fn load_session_from_db(
    &mut self,
    id: i64,
  ) -> SessionManagerResult<()> {
    let mut session_row = match self.db.query(LOAD_SESSION, (id,)).await {
      Ok(rows) => rows,
      Err(e) => {
        return Err(SessionManagerError::SessionLoadFailed(e.to_string()));
      }
    };

    let row = match session_row.next().await {
      Ok(Some(row)) => row,
      _ => {
        return Err(SessionManagerError::SessionLoadFailed(
          "Session not found".to_string(),
        ));
      }
    };

    let session_id: i64 = match row.get(0) {
      Ok(v) => v,
      Err(_) => {
        return Err(SessionManagerError::SessionLoadFailed(
          "Invalid session data".to_string(),
        ));
      }
    };
    let created_at: i64 = match row.get(1) {
      Ok(v) => v,
      Err(_) => {
        return Err(SessionManagerError::SessionLoadFailed(
          "Invalid session data".to_string(),
        ));
      }
    };
    let last_interaction: i64 = match row.get(2) {
      Ok(v) => v,
      Err(_) => {
        return Err(SessionManagerError::SessionLoadFailed(
          "Invalid session data".to_string(),
        ));
      }
    };

    let mut messages_rows =
      match self.db.query(LOAD_MESSAGES, (session_id,)).await {
        Ok(rows) => rows,
        Err(e) => {
          return Err(SessionManagerError::SessionLoadFailed(e.to_string()));
        }
      };

    let mut messages = Vec::new();
    while let Ok(Some(msg_row)) = messages_rows.next().await {
      let role: String = match msg_row.get(2) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let content: String = match msg_row.get(3) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let timestamp: i64 = match msg_row.get(4) {
        Ok(v) => v,
        Err(_) => continue,
      };

      let message = Message {
        role,
        content,
        timestamp,
      };

      messages.push(message);
    }

    let session = Session {
      id: session_id,
      created_at,
      last_interaction,
      messages,
    };

    self.sessions.push(session);
    self.current_session = Some(id);
    return Ok(());
  }

  pub async fn list_sessions(&self) -> SessionManagerResult<Vec<Session>> {
    let mut rows = match self.db.query(LIST_SESSIONS, ()).await {
      Ok(rows) => rows,
      Err(e) => {
        return Err(SessionManagerError::SessionLoadFailed(e.to_string()));
      }
    };

    let mut result = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
      let id: i64 = match row.get(0) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let created_at: i64 = match row.get(1) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let last_interaction: i64 = match row.get(2) {
        Ok(v) => v,
        Err(_) => continue,
      };

      let session = Session {
        id,
        created_at,
        last_interaction,
        messages: Vec::new(),
      };

      result.push(session);
    }

    return Ok(result);
  }

  fn get_session_by_id(&self, id: i64) -> Option<&Session> {
    return self.sessions.iter().find(|session| session.id == id);
  }

  pub fn get_current_session(&self) -> Option<&Session> {
    return self
      .current_session
      .and_then(|id| self.get_session_by_id(id));
  }
}
