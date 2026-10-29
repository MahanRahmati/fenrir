mod queries;
mod repository;
pub mod session;
mod summary;
mod tool_approval;

use anyhow::Result;
use uuid::Uuid;

pub use crate::session_manager::session::Message;
pub use crate::session_manager::summary::Summary;
pub use crate::session_manager::tool_approval::{
  ToolApprovalRequest, ToolApprovalStatus,
};

use crate::database::DB;
use crate::session_manager::repository::SessionRepository;
use crate::session_manager::session::Session;

const CANCELLED_TOOL_RESULT: &str = concat!(
  "Tool call cancelled by the user before completion. ",
  "Do not retry unless the user asks."
);

pub struct SessionManager {
  sessions: Vec<Session>,
  current_session: Option<Uuid>,
  repository: SessionRepository,
}

impl SessionManager {
  pub fn new(db: DB) -> Self {
    return Self {
      sessions: Vec::new(),
      current_session: None,
      repository: SessionRepository::new(db),
    };
  }

  pub async fn create_session(
    &mut self,
    parent_id: Option<Uuid>,
  ) -> Result<Uuid> {
    let session = Session::new(parent_id);
    self.repository.insert_session(&session).await?;
    let id = session.id;
    self.sessions.push(session);
    self.current_session = Some(id);
    return Ok(id);
  }

  pub async fn add_system_message_to_session(
    &mut self,
    session_id: Uuid,
    content: &str,
  ) -> Result<()> {
    self
      .add_message_to_session(session_id, |session| {
        session.add_system_message(content);
      })
      .await?;
    return Ok(());
  }

  pub async fn add_user_message_to_session(
    &mut self,
    session_id: Uuid,
    content: &str,
  ) -> Result<()> {
    self
      .add_message_to_session(session_id, |session| {
        session.add_user_message(content);
      })
      .await?;
    return Ok(());
  }

  pub async fn add_assistant_message_to_session(
    &mut self,
    session_id: Uuid,
    content: &str,
  ) -> Result<()> {
    self
      .add_message_to_session(session_id, |session| {
        session.add_assistant_message(content);
      })
      .await?;
    return Ok(());
  }

  pub async fn add_reasoning_message_to_session(
    &mut self,
    session_id: Uuid,
    content: &str,
  ) -> Result<()> {
    self
      .add_message_to_session(session_id, |session| {
        session.add_reasoning_message(content);
      })
      .await?;
    return Ok(());
  }

  pub async fn add_tool_result_to_session(
    &mut self,
    session_id: Uuid,
    tool_call_id: &str,
    result: &str,
  ) -> Result<()> {
    self
      .add_message_to_session(session_id, |session| {
        session
          .add_tool_result(tool_call_id, &format!("Tool result: {result}"));
      })
      .await?;
    return Ok(());
  }

  pub async fn add_cancelled_tool_result_to_session(
    &mut self,
    session_id: Uuid,
    tool_call_id: &str,
  ) -> Result<()> {
    self
      .add_tool_result_to_session(
        session_id,
        tool_call_id,
        CANCELLED_TOOL_RESULT,
      )
      .await?;
    return Ok(());
  }

  pub async fn add_tool_call_to_session(
    &mut self,
    session_id: Uuid,
    tool_call_id: &str,
    tool_name: &str,
    arguments: &str,
  ) -> Result<()> {
    self
      .add_message_to_session(session_id, |session| {
        session.add_tool_call(tool_call_id, tool_name, arguments);
      })
      .await?;
    return Ok(());
  }

  async fn add_message_to_session<F>(
    &mut self,
    session_id: Uuid,
    f: F,
  ) -> Result<()>
  where
    F: FnOnce(&mut Session),
  {
    if let Some(session) = self.sessions.iter_mut().find(|s| s.id == session_id)
    {
      f(session);
      let new_msg = session.messages.last();
      if let Some(msg) = new_msg {
        self.repository.insert_message(session.id, msg).await?;
      }
    }
    return Ok(());
  }

  pub async fn add_summary_to_session(
    &mut self,
    session_id: Uuid,
    content: &str,
    covers_message_id: Uuid,
  ) -> Result<()> {
    if let Some(session) = self.sessions.iter_mut().find(|s| s.id == session_id)
    {
      session.add_summary(content, covers_message_id);
      let new_summary = session.summaries.last();
      if let Some(summary) = new_summary {
        self.repository.insert_summary(session.id, summary).await?;
      }
    }
    return Ok(());
  }

  pub async fn update_session_title(
    &mut self,
    session_id: Uuid,
    title: &str,
  ) -> Result<()> {
    if let Some(session) = self.sessions.iter_mut().find(|s| s.id == session_id)
    {
      session.title = Some(title.to_string());
      self.repository.update_title(session_id, title).await?;
    }
    return Ok(());
  }

  pub async fn load_session_from_db(&mut self, id: Uuid) -> Result<()> {
    let session = self.repository.load(id).await?;
    self.sessions.push(session);
    self.current_session = Some(id);
    return Ok(());
  }

  pub async fn list_sessions(&self) -> Result<Vec<Session>> {
    return self.repository.list().await;
  }

  pub async fn add_pending_tool_approval(
    &self,
    request: &ToolApprovalRequest,
  ) -> Result<()> {
    self.repository.insert_tool_approval(request).await?;
    return Ok(());
  }

  pub async fn resolve_tool_approval(
    &self,
    tool_call_id: &str,
    status: ToolApprovalStatus,
  ) -> Result<()> {
    self
      .repository
      .resolve_tool_approval(tool_call_id, status)
      .await?;
    return Ok(());
  }

  pub async fn tool_approval_status(
    &self,
    tool_call_id: &str,
  ) -> Result<Option<ToolApprovalStatus>> {
    return self.repository.tool_approval_status(tool_call_id).await;
  }

  fn get_session_by_id(&self, id: Uuid) -> Option<&Session> {
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

  pub fn set_current_session(&mut self, id: Uuid) {
    self.current_session = Some(id);
  }
}
