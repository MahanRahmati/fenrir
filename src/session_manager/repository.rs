use uuid::Uuid;

use crate::database::DB;
use crate::session_manager::errors::{
  SessionManagerError, SessionManagerResult,
};
use crate::session_manager::queries::*;
use crate::session_manager::session::{Message, Session};
use crate::session_manager::summary::Summary;

pub struct SessionRepository {
  db: DB,
}

impl SessionRepository {
  pub fn new(db: DB) -> Self {
    return Self { db };
  }

  pub async fn insert_session(
    &self,
    session: &Session,
  ) -> SessionManagerResult<()> {
    match self
      .db
      .execute(
        INSERT_SESSION,
        (
          session.id.to_string(),
          session.parent_id.map(|id| id.to_string()),
          session.title.clone(),
          session.created_at,
          session.last_interaction,
        ),
      )
      .await
    {
      Ok(_) => Ok(()),
      Err(e) => Err(SessionManagerError::DatabaseError(e.to_string())),
    }
  }

  pub async fn insert_message(
    &self,
    session_id: Uuid,
    msg: &Message,
  ) -> SessionManagerResult<()> {
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
          &*msg.id.to_string(),
          session_id.to_string(),
          &*msg.role,
          &*msg.content,
          msg.timestamp,
          tool_call_id,
          tool_calls,
        ),
      )
      .await
    {
      Ok(_) => Ok(()),
      Err(e) => Err(SessionManagerError::DatabaseError(e.to_string())),
    }
  }

  pub async fn insert_summary(
    &self,
    session_id: Uuid,
    summary: &Summary,
  ) -> SessionManagerResult<()> {
    match self
      .db
      .execute(
        INSERT_SUMMARY,
        (
          &*summary.id.to_string(),
          session_id.to_string(),
          &*summary.content,
          &*summary.covers_message_id.to_string(),
          summary.created_at,
        ),
      )
      .await
    {
      Ok(_) => Ok(()),
      Err(e) => Err(SessionManagerError::DatabaseError(e.to_string())),
    }
  }

  pub async fn update_title(
    &self,
    session_id: Uuid,
    title: &str,
  ) -> SessionManagerResult<()> {
    match self
      .db
      .execute(UPDATE_SESSION_TITLE, (title, session_id.to_string()))
      .await
    {
      Ok(_) => Ok(()),
      Err(e) => Err(SessionManagerError::DatabaseError(e.to_string())),
    }
  }

  pub async fn load(&self, id: Uuid) -> SessionManagerResult<Session> {
    let mut session_row =
      match self.db.query(LOAD_SESSION, (id.to_string(),)).await {
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

    let session_id: String = match row.get(0) {
      Ok(v) => v,
      Err(_) => {
        return Err(SessionManagerError::SessionLoadFailed(
          "Invalid session data".to_string(),
        ));
      }
    };
    let session_id = match Uuid::parse_str(&session_id) {
      Ok(v) => v,
      Err(_) => {
        return Err(SessionManagerError::SessionLoadFailed(
          "Invalid session data".to_string(),
        ));
      }
    };
    let parent_id: String = row.get(1).unwrap_or_default();
    let parent_id = if parent_id.is_empty() {
      None
    } else {
      match Uuid::parse_str(&parent_id) {
        Ok(v) => Some(v),
        Err(_) => {
          return Err(SessionManagerError::SessionLoadFailed(
            "Invalid session data".to_string(),
          ));
        }
      }
    };
    let title: String = row.get(2).unwrap_or_default();
    let title = if title.is_empty() { None } else { Some(title) };
    let created_at: i64 = match row.get(3) {
      Ok(v) => v,
      Err(_) => {
        return Err(SessionManagerError::SessionLoadFailed(
          "Invalid session data".to_string(),
        ));
      }
    };
    let last_interaction: i64 = match row.get(4) {
      Ok(v) => v,
      Err(_) => {
        return Err(SessionManagerError::SessionLoadFailed(
          "Invalid session data".to_string(),
        ));
      }
    };

    let mut messages_rows = match self
      .db
      .query(LOAD_MESSAGES, (session_id.to_string(),))
      .await
    {
      Ok(rows) => rows,
      Err(e) => {
        return Err(SessionManagerError::SessionLoadFailed(e.to_string()));
      }
    };

    let mut messages = Vec::new();
    while let Ok(Some(msg_row)) = messages_rows.next().await {
      let id: String = match msg_row.get(0) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let id = match Uuid::parse_str(&id) {
        Ok(v) => v,
        Err(_) => continue,
      };
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
        id,
        msg_type: None,
        role,
        content,
        tool_call_id,
        tool_calls,
        timestamp,
      };

      messages.push(message);
    }

    let mut summaries_rows = match self
      .db
      .query(LOAD_SUMMARIES, (session_id.to_string(),))
      .await
    {
      Ok(rows) => rows,
      Err(e) => {
        return Err(SessionManagerError::SessionLoadFailed(e.to_string()));
      }
    };

    let mut summaries = Vec::new();
    while let Ok(Some(summary_row)) = summaries_rows.next().await {
      let id: String = match summary_row.get(0) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let id = match Uuid::parse_str(&id) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let content: String = match summary_row.get(2) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let covers_message_id: String = match summary_row.get(3) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let covers_message_id = match Uuid::parse_str(&covers_message_id) {
        Ok(v) => v,
        Err(_) => continue,
      };

      let summary_created_at: i64 = match summary_row.get(4) {
        Ok(v) => v,
        Err(_) => continue,
      };

      let summary = Summary {
        id,
        session_id,
        content,
        covers_message_id,
        created_at: summary_created_at,
      };
      summaries.push(summary);
    }

    let session = Session {
      id: session_id,
      parent_id,
      title,
      created_at,
      last_interaction,
      messages,
      summaries,
    };

    return Ok(session);
  }

  pub async fn list(&self) -> SessionManagerResult<Vec<Session>> {
    let mut rows = match self.db.query(LIST_SESSIONS, ()).await {
      Ok(rows) => rows,
      Err(e) => {
        return Err(SessionManagerError::SessionLoadFailed(e.to_string()));
      }
    };

    let mut result = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
      let id: String = match row.get(0) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let id = match Uuid::parse_str(&id) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let parent_id: String = row.get(1).unwrap_or_default();
      let parent_id = if parent_id.is_empty() {
        None
      } else {
        match Uuid::parse_str(&parent_id) {
          Ok(v) => Some(v),
          Err(_) => continue,
        }
      };
      let title: String = row.get(2).unwrap_or_default();
      let title = if title.is_empty() { None } else { Some(title) };
      let created_at: i64 = match row.get(3) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let last_interaction: i64 = match row.get(4) {
        Ok(v) => v,
        Err(_) => continue,
      };

      let session = Session {
        id,
        parent_id,
        title,
        created_at,
        last_interaction,
        messages: Vec::new(),
        summaries: Vec::new(),
      };

      result.push(session);
    }

    return Ok(result);
  }
}
