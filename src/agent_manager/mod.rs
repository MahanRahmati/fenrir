pub mod agent;
mod compaction;
mod harness;

pub use agent::Agent;

use std::sync::Arc;

use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

use crate::app::AppEvent;
use crate::app::command::{AppCommand, AppCommandResult};
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::config::Config;
use crate::database::DB;
use crate::prompt::Prompt;
use crate::provider_manager::{ChatUsage, ProviderManager};
use crate::session_manager::SessionManager;
use crate::slash_commands::SlashCommand;
use crate::vlog;

pub struct AgentManager {
  pub session_manager: SessionManager,
  pub provider_manager: Arc<ProviderManager>,
  pub config: Config,
  pub current: Agent,
}

impl AgentManager {
  pub fn new(config: Config, db: DB) -> Self {
    let session_manager = SessionManager::new(db);
    let provider_manager = Arc::new(ProviderManager::new(config.clone()));
    let current = Agent::new(config.clone(), provider_manager.clone(), None);
    return AgentManager {
      session_manager,
      provider_manager,
      config,
      current,
    };
  }

  pub fn last_usage(&self) -> Option<ChatUsage> {
    return self.current.last_usage;
  }

  pub async fn ensure_session_with_system_message(
    &mut self,
  ) -> RuntimeResult<()> {
    if self.session_manager.get_current_session().is_some() {
      return Ok(());
    }

    let session_id = self
      .session_manager
      .create_session(None)
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;
    vlog!("Created session with ID: {}", session_id);

    let prompt =
      Prompt::new(&self.config, self.current.persona_manager.current());
    vlog!("Prompt: {}", prompt.get_system_prompt());
    self
      .session_manager
      .add_system_message_to_session(session_id, prompt.get_system_prompt())
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;

    return Ok(());
  }

  pub async fn process_chat(
    &mut self,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    return self
      .current
      .process_chat(&mut self.session_manager, tx, cancel)
      .await;
  }

  pub async fn handle_command(
    &mut self,
    command: AppCommand,
    cancel: &CancellationToken,
  ) -> RuntimeResult<AppCommandResult> {
    let has_session = self.session_manager.get_current_session().is_some();
    return match command {
      AppCommand::Exit => Ok(AppCommandResult::Exit),
      AppCommand::Help => Ok(AppCommandResult::Help(
        SlashCommand::available_commands(has_session),
      )),
      AppCommand::ListSessions => {
        let sessions = self
          .session_manager
          .list_sessions()
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
        let current = self.session_manager.get_current_session().map(|s| s.id);
        return Ok(AppCommandResult::Sessions { sessions, current });
      }
      AppCommand::LoadSession { id } => {
        self
          .session_manager
          .load_session_from_db(id)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
        self.current.tool_manager.reset_cache();
        let Some(session) = self.session_manager.get_current_session().cloned()
        else {
          return Err(RuntimeError::Session("No active session".into()));
        };
        return Ok(AppCommandResult::Session(session));
      }
      AppCommand::ListPersonas => {
        if self.session_manager.get_current_session().is_some() {
          return Ok(AppCommandResult::Noop);
        }
        let personas = self.current.persona_manager.list();
        let current = self.current.persona_manager.current().name().to_string();
        return Ok(AppCommandResult::Personas { personas, current });
      }
      AppCommand::ListModels => {
        let models = self.provider_manager.list_models();
        let current = self.provider_manager.active_model();
        return Ok(AppCommandResult::Models { models, current });
      }
      AppCommand::SelectModel { model } => {
        self.provider_manager.set_active_model(&model);
        return Ok(AppCommandResult::Model(model));
      }
      AppCommand::SelectPersona { name } => {
        self
          .current
          .persona_manager
          .set(&name)
          .map_err(RuntimeError::Persona)?;
        return Ok(AppCommandResult::Noop);
      }
      AppCommand::Compact => {
        let summary = self.compact(cancel).await?;
        return Ok(AppCommandResult::Compacted { summary });
      }
      AppCommand::New => {
        self.session_manager.clear_current_session();
        self.current.tool_manager.reset_cache();
        self.current.persona_manager.reset();
        return Ok(AppCommandResult::Persona(
          self.current.persona_manager.current().name().to_string(),
        ));
      }
    };
  }
}
