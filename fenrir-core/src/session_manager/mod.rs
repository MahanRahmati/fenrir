mod compaction;
mod harness;
mod queries;
pub mod repository;
pub mod session;
mod summary;
mod tool_approval;

use anyhow::{Context, Result};
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::app::{AppEvent, ApprovalChannel};
use crate::config::Config;
use crate::database::DB;
use crate::persona_manager::{Persona, PersonaManager};
use crate::provider_manager::{ChatUsage, Model, ProviderManager};
pub use crate::session_manager::repository::SessionRepository;
pub use crate::session_manager::session::Message;
use crate::session_manager::session::Session;
pub use crate::session_manager::summary::Summary;
pub use crate::session_manager::tool_approval::{
  ToolApprovalRequest, ToolApprovalStatus,
};

pub struct SessionManager {
  sessions: Vec<Session>,
  providers: ProviderManager,
  config: Config,
  repository: SessionRepository,
  model: Option<Model>,
  next_persona: Option<Persona>,
}

impl SessionManager {
  pub fn new(config: Config, db: DB) -> Self {
    let providers = ProviderManager::new(config.clone());
    let model = providers.default_model();
    return Self {
      sessions: Vec::new(),
      providers,
      config,
      repository: SessionRepository::new(db),
      model,
      next_persona: None,
    };
  }

  pub fn get_providers(&self) -> &ProviderManager {
    return &self.providers;
  }

  pub fn persona_name(&self, session_id: Option<Uuid>) -> String {
    return session_id
      .and_then(|id| self.session(id))
      .map(|session| session.persona_manager.current().name().to_string())
      .unwrap_or_else(|| self.next_persona_or_default().name().to_string());
  }

  pub fn current_model(&self, session_id: Option<Uuid>) -> Option<Model> {
    return session_id
      .and_then(|id| self.session(id))
      .and_then(|session| session.model.clone())
      .or_else(|| self.model.clone());
  }

  pub async fn create_session(&mut self) -> Result<Uuid> {
    let mut session = Session::new(
      None,
      self.model.clone(),
      &self.config,
      ApprovalChannel::new(),
    );
    session
      .persona_manager
      .set_persona(self.next_persona_or_default());
    self.repository.insert_session(&session).await?;
    let id = session.id;
    self.sessions.push(session);
    return Ok(id);
  }

  pub async fn load_session(&mut self, id: Uuid) -> Result<()> {
    if let Some(session) = self.session_mut(id) {
      session.tool_manager.reset_cache();
      return Ok(());
    }
    let model = self.model.clone();
    let config = &self.config;
    let session = self
      .repository
      .load(id, |row| {
        Session::from_row(row, config, model.clone(), ApprovalChannel::new())
      })
      .await?;
    self.sessions.push(session);
    return Ok(());
  }

  pub async fn list_sessions(&self) -> Result<Vec<Session>> {
    let model = self.model.clone();
    let config = &self.config;
    return self
      .repository
      .list(|row| {
        Session::from_row(row, config, model.clone(), ApprovalChannel::new())
      })
      .await;
  }

  pub async fn ensure_system_message(&mut self, id: Uuid) -> Result<()> {
    let index = self.index_of(id)?;
    let session = &mut self.sessions[index];
    return session
      .ensure_system_message(&self.config, &self.repository)
      .await;
  }

  pub async fn add_user_message(
    &mut self,
    id: Uuid,
    content: &str,
  ) -> Result<()> {
    let index = self.index_of(id)?;
    let session = &mut self.sessions[index];
    session.add_user_message(content);
    return session.persist_last_message(&self.repository).await;
  }

  pub async fn run_chat(
    &mut self,
    id: Uuid,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> Result<String> {
    let index = self.index_of(id)?;
    let session = &mut self.sessions[index];
    return session
      .harness_loop(&self.config, &self.providers, &self.repository, tx, cancel)
      .await;
  }

  pub async fn maybe_compact(
    &mut self,
    id: Uuid,
    cancel: &CancellationToken,
  ) -> Result<String> {
    let index = self.index_of(id)?;
    let session = &mut self.sessions[index];
    return session
      .maybe_compact(&self.config, &self.providers, &self.repository, cancel)
      .await;
  }

  pub async fn compact(
    &mut self,
    id: Uuid,
    cancel: &CancellationToken,
  ) -> Result<String> {
    let index = self.index_of(id)?;
    let session = &mut self.sessions[index];
    return session
      .compact(&self.providers, &self.repository, cancel)
      .await;
  }

  pub fn last_usage(&self, id: Uuid) -> Option<ChatUsage> {
    return self.session(id).and_then(|session| session.last_usage);
  }

  pub fn session(&self, id: Uuid) -> Option<&Session> {
    return self.sessions.iter().find(|session| session.id == id);
  }

  pub fn session_mut(&mut self, id: Uuid) -> Option<&mut Session> {
    return self.sessions.iter_mut().find(|session| session.id == id);
  }

  pub fn set_model(&mut self, session_id: Option<Uuid>, model: Model) {
    if let Some(id) = session_id
      && let Some(session) = self.session_mut(id)
    {
      session.model = Some(model.clone());
    }
    self.model = Some(model);
  }

  pub fn set_persona(
    &mut self,
    session_id: Option<Uuid>,
    name: &str,
  ) -> Result<()> {
    if let Some(id) = session_id
      && let Some(session) = self.session_mut(id)
    {
      session.persona_manager.set(name)?;
    } else {
      self.next_persona = Some(PersonaManager::find(name)?);
    }
    return Ok(());
  }

  pub fn reset_persona(&mut self) {
    self.next_persona = None;
  }

  fn next_persona_or_default(&self) -> Persona {
    return self
      .next_persona
      .clone()
      .unwrap_or_else(|| PersonaManager::new().current().clone());
  }

  fn index_of(&self, id: Uuid) -> Result<usize> {
    return self
      .sessions
      .iter()
      .position(|session| session.id == id)
      .context(format!("Session '{id}' not found"));
  }
}
