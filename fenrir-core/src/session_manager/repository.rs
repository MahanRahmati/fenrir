use anyhow::{Context, Result, bail};
use chrono::Utc;
use uuid::Uuid;

use crate::database::DB;
use crate::engine::protocol::SessionSummary;
use crate::provider_manager::Model;
use crate::session_manager::queries::*;
use crate::session_manager::session::{
  Message, QueuedMessage, QueuedMessageStatus, Session,
};
use crate::session_manager::summary::Summary;
use crate::session_manager::tool_approval::{
  ToolApprovalRequest, ToolApprovalStatus,
};

pub(crate) struct SessionRow {
  pub(crate) id: Uuid,
  pub(crate) parent_id: Option<Uuid>,
  pub(crate) title: Option<String>,
  pub(crate) created_at: i64,
  pub(crate) last_interaction: i64,
  pub(crate) persona: String,
  pub(crate) model: Option<Model>,
  pub(crate) cwd: String,
  pub(crate) messages: Vec<Message>,
  pub(crate) summaries: Vec<Summary>,
}

pub struct SessionRepository {
  db: DB,
}

impl SessionRepository {
  pub fn new(db: DB) -> Self {
    return Self { db };
  }

  pub async fn insert_session(&self, session: &Session) -> Result<()> {
    let model = match &session.model {
      Some(model) => serde_json::to_string(model)
        .context("Failed to serialize session model")?,
      None => String::new(),
    };
    self
      .db
      .execute(
        INSERT_SESSION,
        (
          session.id.to_string(),
          session.parent_id.map(|id| id.to_string()),
          session.title.clone(),
          session.created_at,
          session.last_interaction,
          session.persona.as_str(),
          model,
          session.cwd.as_str(),
        ),
      )
      .await
      .context("Failed to insert session")?;
    return Ok(());
  }

  pub async fn insert_message(
    &self,
    session_id: Uuid,
    msg: &Message,
  ) -> Result<()> {
    let tool_call_id = msg.tool_call_id.clone().unwrap_or_default();
    let tool_calls = match &msg.tool_calls {
      Some(calls) => serde_json::to_string(calls).unwrap_or_default(),
      None => String::new(),
    };
    self
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
      .context(format!("Failed to insert message '{}'", msg.id))?;
    return Ok(());
  }

  pub async fn insert_summary(
    &self,
    session_id: Uuid,
    summary: &Summary,
  ) -> Result<()> {
    self
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
      .context(format!("Failed to insert summary '{}'", summary.id))?;
    return Ok(());
  }

  pub async fn update_title(
    &self,
    session_id: Uuid,
    title: &str,
  ) -> Result<()> {
    self
      .db
      .execute(UPDATE_SESSION_TITLE, (title, session_id.to_string()))
      .await
      .context(format!("Failed to update title for session '{session_id}'"))?;
    return Ok(());
  }

  pub async fn set_persona(
    &self,
    session_id: Uuid,
    persona: &str,
  ) -> Result<()> {
    self
      .db
      .execute(UPDATE_SESSION_PERSONA, (persona, session_id.to_string()))
      .await
      .context(format!(
        "Failed to update persona for session '{session_id}'"
      ))?;
    return Ok(());
  }

  pub async fn set_model(&self, session_id: Uuid, model: &Model) -> Result<()> {
    let model = serde_json::to_string(model)
      .context("Failed to serialize session model")?;
    self
      .db
      .execute(UPDATE_SESSION_MODEL, (model, session_id.to_string()))
      .await
      .context(format!("Failed to update model for session '{session_id}'"))?;
    return Ok(());
  }

  pub(crate) async fn load<F>(&self, id: Uuid, build: F) -> Result<Session>
  where
    F: FnOnce(SessionRow) -> Session,
  {
    let mut session_row = self
      .db
      .query(LOAD_SESSION, (id.to_string(),))
      .await
      .context(format!("Failed to load session '{id}'"))?;

    let row = match session_row.next().await {
      Ok(Some(row)) => row,
      _ => {
        bail!("Session not found");
      }
    };

    let session_id: String = match row.get(0) {
      Ok(v) => v,
      Err(_) => {
        bail!("Invalid session data");
      }
    };
    let session_id = match Uuid::parse_str(&session_id) {
      Ok(v) => v,
      Err(_) => {
        bail!("Invalid session data");
      }
    };
    let parent_id: String = row.get(1).unwrap_or_default();
    let parent_id = if parent_id.is_empty() {
      None
    } else {
      match Uuid::parse_str(&parent_id) {
        Ok(v) => Some(v),
        Err(_) => {
          bail!("Invalid session data");
        }
      }
    };
    let title: String = row.get(2).unwrap_or_default();
    let title = if title.is_empty() { None } else { Some(title) };
    let created_at: i64 = match row.get(3) {
      Ok(v) => v,
      Err(_) => {
        bail!("Invalid session data");
      }
    };
    let last_interaction: i64 = match row.get(4) {
      Ok(v) => v,
      Err(_) => {
        bail!("Invalid session data");
      }
    };
    let persona: String = row.get(5).unwrap_or_default();
    let persona = if persona.is_empty() {
      String::new()
    } else {
      persona
    };
    let model: String = row.get(6).unwrap_or_default();
    let model = if model.is_empty() {
      None
    } else {
      serde_json::from_str(&model).ok()
    };
    let cwd: String = row.get(7).unwrap_or_default();
    let cwd = if cwd.is_empty() { String::new() } else { cwd };

    let mut messages_rows = self
      .db
      .query(LOAD_MESSAGES, (session_id.to_string(),))
      .await
      .context(format!(
        "Failed to load messages for session '{session_id}'"
      ))?;

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

    let mut summaries_rows = self
      .db
      .query(LOAD_SUMMARIES, (session_id.to_string(),))
      .await
      .context(format!(
        "Failed to load summaries for session '{session_id}'"
      ))?;

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

    let row = SessionRow {
      id: session_id,
      parent_id,
      title,
      created_at,
      last_interaction,
      persona,
      model,
      cwd,
      messages,
      summaries,
    };

    return Ok(build(row));
  }

  pub async fn insert_tool_approval(
    &self,
    request: &ToolApprovalRequest,
  ) -> Result<()> {
    self
      .db
      .execute(
        INSERT_TOOL_APPROVAL,
        (
          request.tool_call_id.as_str(),
          request.session_id.to_string(),
          request.turn_id.map(|id| id.to_string()),
          request.tool_name.as_str(),
          request.message.as_str(),
          request.created_at,
        ),
      )
      .await
      .context(format!(
        "Failed to insert tool approval '{}'",
        request.tool_call_id
      ))?;
    return Ok(());
  }

  pub async fn load_pending_tool_approvals(
    &self,
    session_id: Uuid,
  ) -> Result<Vec<ToolApprovalRequest>> {
    let mut rows = self
      .db
      .query(LOAD_PENDING_TOOL_APPROVALS, (session_id.to_string(),))
      .await
      .context(format!(
        "Failed to load pending tool approvals for session '{session_id}'"
      ))?;

    let mut approvals = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
      let tool_call_id: String = match row.get(0) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let turn_id: Option<String> = row.get(2).ok();
      let turn_id = turn_id.and_then(|id| Uuid::parse_str(&id).ok());
      let tool_name: String = match row.get(3) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let message: String = match row.get(4) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let created_at: i64 = match row.get(6) {
        Ok(v) => v,
        Err(_) => continue,
      };

      approvals.push(ToolApprovalRequest {
        tool_call_id,
        session_id,
        turn_id,
        tool_name,
        message,
        created_at,
      });
    }

    return Ok(approvals);
  }

  pub async fn resolve_tool_approval(
    &self,
    tool_call_id: &str,
    status: ToolApprovalStatus,
  ) -> Result<()> {
    let decided_at = Utc::now().timestamp();
    self
      .db
      .execute(
        RESOLVE_TOOL_APPROVAL,
        (status.as_str(), decided_at, tool_call_id),
      )
      .await
      .context(format!("Failed to resolve tool approval '{tool_call_id}'"))?;
    return Ok(());
  }

  pub async fn insert_queued_message(
    &self,
    session_id: Uuid,
    content: &str,
  ) -> Result<QueuedMessage> {
    let message = QueuedMessage {
      id: Uuid::now_v7(),
      session_id,
      content: content.to_string(),
      created_at: Utc::now().timestamp(),
    };
    self
      .db
      .execute(
        INSERT_QUEUED_MESSAGE,
        (
          message.id.to_string(),
          session_id.to_string(),
          message.content.as_str(),
          QueuedMessageStatus::Pending.as_str(),
          message.created_at,
        ),
      )
      .await
      .context("Failed to insert queued message")?;
    return Ok(message);
  }

  pub async fn load_queued_messages(
    &self,
    session_id: Uuid,
  ) -> Result<Vec<QueuedMessage>> {
    let mut rows = self
      .db
      .query(
        LOAD_QUEUED_MESSAGES,
        (
          session_id.to_string(),
          QueuedMessageStatus::Pending.as_str(),
        ),
      )
      .await
      .context(format!(
        "Failed to load queued messages for session '{session_id}'"
      ))?;

    let mut messages = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
      let id: String = match row.get(0) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let id = match Uuid::parse_str(&id) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let _session_id: String = match row.get(1) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let content: String = match row.get(2) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let created_at: i64 = match row.get(4) {
        Ok(v) => v,
        Err(_) => continue,
      };

      messages.push(QueuedMessage {
        id,
        session_id,
        content,
        created_at,
      });
    }

    return Ok(messages);
  }

  pub async fn has_queued_messages(&self, session_id: Uuid) -> Result<bool> {
    let mut rows = self
      .db
      .query(HAS_QUEUED_MESSAGES, (session_id.to_string(),))
      .await
      .context(format!(
        "Failed to check queued messages for session '{session_id}'"
      ))?;
    return Ok(matches!(rows.next().await, Ok(Some(_))));
  }

  pub async fn claim_next_queued_message(
    &self,
    session_id: Uuid,
  ) -> Result<Option<QueuedMessage>> {
    let mut rows = self
      .db
      .query(
        LOAD_NEXT_QUEUED_MESSAGE,
        (
          session_id.to_string(),
          QueuedMessageStatus::Pending.as_str(),
        ),
      )
      .await
      .context(format!(
        "Failed to load next queued message for session '{session_id}'"
      ))?;

    let message = match rows.next().await {
      Ok(Some(row)) => {
        let id: String = match row.get(0) {
          Ok(v) => v,
          Err(_) => return Ok(None),
        };
        let id = match Uuid::parse_str(&id) {
          Ok(v) => v,
          Err(_) => return Ok(None),
        };
        let content: String = match row.get(2) {
          Ok(v) => v,
          Err(_) => return Ok(None),
        };
        let created_at: i64 = match row.get(4) {
          Ok(v) => v,
          Err(_) => return Ok(None),
        };

        let message = QueuedMessage {
          id,
          session_id,
          content,
          created_at,
        };

        self
          .db
          .execute(
            CLAIM_QUEUED_MESSAGE,
            (
              QueuedMessageStatus::Processing.as_str(),
              id.to_string(),
              QueuedMessageStatus::Pending.as_str(),
            ),
          )
          .await
          .context(format!("Failed to claim queued message '{id}'"))?;

        message
      }
      _ => return Ok(None),
    };

    return Ok(Some(message));
  }

  pub async fn complete_queued_message(&self, id: Uuid) -> Result<()> {
    self
      .db
      .execute(DELETE_QUEUED_MESSAGE, (id.to_string(),))
      .await
      .context(format!("Failed to complete queued message '{id}'"))?;
    return Ok(());
  }

  pub async fn release_queued_message(&self, id: Uuid) -> Result<()> {
    self
      .db
      .execute(
        RELEASE_QUEUED_MESSAGE,
        (
          QueuedMessageStatus::Pending.as_str(),
          id.to_string(),
          QueuedMessageStatus::Processing.as_str(),
        ),
      )
      .await
      .context(format!("Failed to release queued message '{id}'"))?;
    return Ok(());
  }

  pub async fn reset_processing_queued_messages(
    &self,
    session_id: Uuid,
  ) -> Result<()> {
    self
      .db
      .execute(
        RESET_PROCESSING_QUEUED_MESSAGES,
        (
          QueuedMessageStatus::Pending.as_str(),
          session_id.to_string(),
          QueuedMessageStatus::Processing.as_str(),
        ),
      )
      .await
      .context(format!(
        "Failed to reset processing queued messages for session '{session_id}'"
      ))?;
    return Ok(());
  }

  pub async fn clear_queued_messages(&self, session_id: Uuid) -> Result<()> {
    self
      .db
      .execute(CLEAR_QUEUED_MESSAGES, (session_id.to_string(),))
      .await
      .context(format!(
        "Failed to clear queued messages for session '{session_id}'"
      ))?;
    return Ok(());
  }

  pub async fn list_summaries(
    &self,
    limit: Option<u32>,
  ) -> Result<Vec<SessionSummary>> {
    let limit = limit.map(i64::from).unwrap_or(-1);
    let mut rows = self
      .db
      .query(LIST_SESSIONS, (limit,))
      .await
      .context("Failed to list sessions")?;

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

      result.push(SessionSummary {
        id,
        parent_id,
        title,
        created_at,
        last_interaction,
      });
    }

    return Ok(result);
  }
}
