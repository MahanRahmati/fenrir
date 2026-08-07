mod errors;
mod queries;
pub mod session;

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

  pub async fn create_session(&mut self) -> SessionManagerResult<i64> {
    let session = Session::new();
    match self
      .db
      .execute(
        INSERT_SESSION,
        (
          session.id,
          session.title.clone(),
          session.created_at,
          session.last_interaction,
        ),
      )
      .await
    {
      Ok(_) => {}
      Err(e) => {
        return Err(SessionManagerError::DatabaseError(e.to_string()));
      }
    };

    let id = session.id;
    self.sessions.push(session);
    self.current_session = Some(id);
    return Ok(id);
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

  pub async fn add_reasoning_message_to_current_session(
    &mut self,
    content: &str,
  ) -> SessionManagerResult<()> {
    self
      .add_message_to_current_session(|session| {
        session.add_reasoning_message(content);
      })
      .await?;
    return Ok(());
  }

  pub async fn add_tool_result_to_current_session(
    &mut self,
    tool_call_id: &str,
    result: &str,
  ) -> SessionManagerResult<()> {
    self
      .add_message_to_current_session(|session| {
        session
          .add_tool_result(tool_call_id, &format!("Tool result: {result}"));
      })
      .await?;
    return Ok(());
  }

  pub async fn add_tool_call_to_current_session(
    &mut self,
    tool_call_id: &str,
    tool_name: &str,
    arguments: &str,
  ) -> SessionManagerResult<()> {
    self
      .add_message_to_current_session(|session| {
        session.add_tool_call(tool_call_id, tool_name, arguments);
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
      let new_msg = session.messages.last();
      if let Some(msg) = new_msg {
        let tool_call_id = msg.tool_call_id.clone().unwrap_or_default();
        let tool_calls = match &msg.tool_calls {
          Some(calls) => serde_json::to_string(calls).unwrap_or_default(),
          None => String::new(),
        };
        match self
          .db
          .execute(
            INSERT_MESSAGE,
            (
              session.id,
              &*msg.role,
              &*msg.content,
              msg.timestamp,
              tool_call_id,
              tool_calls,
            ),
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

  pub async fn update_current_session_title(
    &mut self,
    title: &str,
  ) -> SessionManagerResult<()> {
    let session_id = match self.current_session {
      Some(id) => id,
      None => return Err(SessionManagerError::NoCurrentSession),
    };
    if let Some(session) = self.sessions.iter_mut().find(|s| s.id == session_id)
    {
      session.title = Some(title.to_string());
      match self
        .db
        .execute(UPDATE_SESSION_TITLE, (title, session_id))
        .await
      {
        Ok(_) => {}
        Err(e) => {
          return Err(SessionManagerError::DatabaseError(e.to_string()));
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
    let title: String = match row.get(1) {
      Ok(v) => v,
      Err(_) => {
        return Err(SessionManagerError::SessionLoadFailed(
          "Invalid session data".to_string(),
        ));
      }
    };
    let title = if title.is_empty() { None } else { Some(title) };
    let created_at: i64 = match row.get(2) {
      Ok(v) => v,
      Err(_) => {
        return Err(SessionManagerError::SessionLoadFailed(
          "Invalid session data".to_string(),
        ));
      }
    };
    let last_interaction: i64 = match row.get(3) {
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
      let tool_call_id: String = msg_row.get(5).unwrap_or_default();
      let tool_calls: String = msg_row.get(6).unwrap_or_default();

      let tool_call_id = if tool_call_id.is_empty() {
        None
      } else {
        Some(tool_call_id)
      };
      let tool_calls = if tool_calls.is_empty() {
        None
      } else {
        serde_json::from_str(&tool_calls).ok()
      };

      let message = Message {
        msg_type: None,
        role,
        content,
        tool_call_id,
        tool_calls,
        timestamp,
      };

      messages.push(message);
    }

    let session = Session {
      id: session_id,
      created_at,
      last_interaction,
      title,
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
      let title: String = match row.get(1) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let title = if title.is_empty() { None } else { Some(title) };
      let created_at: i64 = match row.get(2) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let last_interaction: i64 = match row.get(3) {
        Ok(v) => v,
        Err(_) => continue,
      };

      let session = Session {
        id,
        created_at,
        last_interaction,
        title,
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

  pub fn clear_current_session(&mut self) {
    self.current_session = None;
  }
}
