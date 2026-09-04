mod chat;
mod cli;
mod command;
mod compaction;
mod errors;
mod event;
mod harness;
mod titles;
mod tui;

pub use event::{AppEvent, EventSink};
use tokio::sync::mpsc::Sender;

use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::app::command::{AppCommand, AppCommandResult};
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::config::Config;
use crate::database::DB;
use crate::persona_manager::PersonaManager;
use crate::prompt::Prompt;
use crate::provider_manager::ChatUsage;
use crate::provider_manager::ProviderManager;
use crate::provider_manager::ToolCall;
use crate::session_manager::SessionManager;
use crate::shutdown::Shutdown;
use crate::slash_commands::SlashCommand;
use crate::tool_manager::ToolApproval;
use crate::tool_manager::ToolManager;
use crate::tool_manager::ToolRejection;
use crate::tool_manager::tool::parse_tool_params;
use crate::vlog;

pub struct App {
  session_manager: SessionManager,
  provider_manager: ProviderManager,
  tool_manager: ToolManager,
  persona_manager: PersonaManager,
  shutdown: Shutdown,
  config: Config,
  last_usage: Option<ChatUsage>,
}

impl App {
  pub fn new(config: Config, db: DB) -> Self {
    let session_manager = SessionManager::new(db);
    let tool_manager = ToolManager::new(&config);
    let provider_manager = ProviderManager::new(config.clone());
    let persona_manager = PersonaManager::new();
    let shutdown = Shutdown::new();
    return App {
      session_manager,
      provider_manager,
      tool_manager,
      persona_manager,
      shutdown,
      config,
      last_usage: None,
    };
  }

  async fn ensure_session_with_system_message(&mut self) -> RuntimeResult<()> {
    if self.session_manager.get_current_session().is_some() {
      return Ok(());
    }

    let session_id = self
      .session_manager
      .create_session()
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;
    vlog!("Created session with ID: {}", session_id);

    let prompt = Prompt::new(&self.config, self.persona_manager.current());
    vlog!("Prompt: {}", prompt.get_system_prompt());
    self
      .session_manager
      .add_system_message_to_current_session(prompt.get_system_prompt())
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;

    return Ok(());
  }

  pub fn last_usage(&self) -> Option<ChatUsage> {
    return self.last_usage;
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
        self.tool_manager.reset_cache();
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
        let personas = self.persona_manager.list();
        let current = self.persona_manager.current().name().to_string();
        return Ok(AppCommandResult::Personas { personas, current });
      }
      AppCommand::SelectPersona { name } => {
        self
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
        self.tool_manager.reset_cache();
        self.persona_manager.reset();
        return Ok(AppCommandResult::Persona(
          self.persona_manager.current().name().to_string(),
        ));
      }
    };
  }

  async fn execute_tool_calls(
    &mut self,
    tool_calls: &[ToolCall],
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> RuntimeResult<()> {
    for tool_call in tool_calls {
      if cancel.is_cancelled() {
        break;
      }
      vlog!(
        "Tool call: {} with arguments: {}",
        tool_call.name,
        tool_call.arguments
      );

      self
        .session_manager
        .add_tool_call_to_current_session(
          &tool_call.id,
          &tool_call.name,
          &tool_call.arguments,
        )
        .await
        .map_err(|e| RuntimeError::Session(e.to_string()))?;

      if self.config.general.show_tools {
        tx.send(AppEvent::ToolCall {
          name: tool_call.name.clone(),
          arguments: tool_call.arguments.clone(),
        })
        .await
        .ok();
      }

      let params = parse_tool_params(&tool_call.arguments);

      let tool_access = self.persona_manager.current().allowed_tools();
      let approval = self.tool_manager.approve_tool_call(
        &tool_call.name,
        &params,
        tool_access,
      );
      let rejection = match approval {
        ToolApproval::Allowed => None,
        ToolApproval::NeedsConfirmation { message } => {
          let (reply_tx, reply_rx) = oneshot::channel();
          tx.send(AppEvent::ToolConfirmation {
            message,
            reply: reply_tx,
          })
          .await
          .ok();
          match reply_rx.await {
            Ok(true) => None,
            _ => Some(ToolRejection::UserDeclined),
          }
        }
        ToolApproval::Rejected(rejection) => Some(rejection),
      };
      if let Some(rejection) = rejection {
        let (result, error) = match rejection {
          ToolRejection::NotAllowedForPersona => (
            format!(
              "Tool '{}' is not available for the current persona. Use only the tools available to this persona.",
              tool_call.name
            ),
            true,
          ),
          ToolRejection::CwdRestriction { path } => (
            format!(
              "Tool '{}' was blocked: the target path '{}' is outside the current working directory. Tools are restricted to the current working directory; retry with a path inside it.",
              tool_call.name, path
            ),
            true,
          ),
          ToolRejection::UserDeclined => (
            format!(
              "Tool '{}' was cancelled by the user. Respect the user's decision; do not retry the same action unless the user asks.",
              tool_call.name
            ),
            false,
          ),
        };
        self
          .session_manager
          .add_tool_result_to_current_session(&tool_call.id, &result)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
        if self.config.general.show_tools {
          tx.send(AppEvent::ToolResult { result, error }).await.ok();
        }
        continue;
      }

      let result = self
        .tool_manager
        .execute_tool(&tool_call.name, params)
        .await;

      let (result_str, error) = match result {
        Ok(r) => {
          vlog!("Tool '{}' result: {}", tool_call.name, r);
          (r, false)
        }
        Err(e) => {
          vlog!("Tool '{}' error: {}", tool_call.name, e);
          (format!("Tool error: {e}"), true)
        }
      };

      self
        .session_manager
        .add_tool_result_to_current_session(&tool_call.id, &result_str)
        .await
        .map_err(|e| RuntimeError::Session(e.to_string()))?;

      if self.config.general.show_tools {
        tx.send(AppEvent::ToolResult {
          result: result_str,
          error,
        })
        .await
        .ok();
      }
    }
    return Ok(());
  }
}
