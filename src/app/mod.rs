mod cli;
mod command;
mod errors;
mod event;
mod tui;

pub use event::{AppEvent, EventSink};
use uuid::Uuid;

use std::collections::HashMap;

use futures::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::app::command::{AppCommand, AppCommandResult};
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::config::Config;
use crate::database::DB;
use crate::mentions::strip_mentions;
use crate::persona_manager::PersonaManager;
use crate::persona_manager::TITLES_PERSONA;
use crate::prompt::Prompt;
use crate::provider_manager::ChatUsage;
use crate::provider_manager::ProviderManager;
use crate::provider_manager::ToolCall;
use crate::session_manager::Message;
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

  pub fn available_commands(&self) -> Vec<SlashCommand> {
    let has_session = self.session_manager.get_current_session().is_some();
    return SlashCommand::all()
      .iter()
      .filter(|cmd| !(matches!(cmd, SlashCommand::Personas) && has_session))
      .cloned()
      .collect();
  }

  pub fn last_usage(&self) -> Option<ChatUsage> {
    return self.last_usage;
  }

  pub async fn handle_command(
    &mut self,
    command: AppCommand,
  ) -> RuntimeResult<AppCommandResult> {
    return match command {
      AppCommand::Exit => Ok(AppCommandResult::Exit),
      AppCommand::Help => Ok(AppCommandResult::Help(self.available_commands())),
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

  pub async fn process_chat(
    &mut self,
    sink: &mut impl EventSink,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    if cancel.is_cancelled() {
      return Ok(String::new());
    }
    self.generate_session_title().await?;
    if self.config.general.stream {
      return self.process_chat_stream(sink, cancel).await;
    }
    return self.process_chat_non_streaming(sink, cancel).await;
  }

  async fn generate_session_title(&mut self) -> RuntimeResult<()> {
    let session = match self.session_manager.get_current_session() {
      Some(session) if session.title.is_none() => session.clone(),
      _ => return Ok(()),
    };
    let first_message = session
      .messages
      .iter()
      .find(|m| m.role == "user")
      .map(|m| m.content.clone())
      .map(|content| strip_mentions(&content))
      .unwrap_or_default();
    if first_message.is_empty() {
      return Ok(());
    }

    let prompt = Prompt::new(&self.config, &TITLES_PERSONA);
    let now = chrono::Utc::now().timestamp();
    let messages = vec![
      Message {
        id: Uuid::now_v7(),
        msg_type: None,
        role: "system".to_string(),
        content: prompt.get_system_prompt().to_string(),
        tool_call_id: None,
        tool_calls: None,
        timestamp: now,
      },
      Message {
        id: Uuid::now_v7(),
        msg_type: None,
        role: "user".to_string(),
        content: first_message,
        tool_call_id: None,
        tool_calls: None,
        timestamp: now,
      },
    ];

    let response = match self.provider_manager.chat(&messages, &[]).await {
      Ok(response) => response,
      Err(e) => {
        vlog!("Failed to generate session title: {e}");
        return Ok(());
      }
    };
    let Some(content) = response.content else {
      return Ok(());
    };
    let title = content
      .trim()
      .trim_matches(|c| c == '"' || c == '\'' || c == '`')
      .to_string();
    if title.is_empty() {
      return Ok(());
    }
    vlog!("Generated session title: {title}");
    self
      .session_manager
      .update_current_session_title(&title)
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;
    return Ok(());
  }

  async fn process_chat_stream(
    &mut self,
    sink: &mut impl EventSink,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    let mut assistant_content = String::new();

    loop {
      let session = self
        .session_manager
        .get_current_session()
        .ok_or(RuntimeError::Session("No active session".into()))?;

      let messages = &session.build_chat_context();
      let tools = self
        .tool_manager
        .get_tools_specs(self.persona_manager.current().allowed_tools());

      let mut stream = self
        .provider_manager
        .chat_stream(messages, &tools)
        .await
        .map_err(|e| RuntimeError::Provider(e.to_string()))?;

      let mut content_buffer = String::new();
      let mut reasoning_buffer = String::new();
      let mut stream_usage: Option<ChatUsage> = None;
      let mut tool_call_builders: HashMap<usize, ToolCall> = HashMap::new();

      loop {
        let delta = tokio::select! {
          biased;
          _ = cancel.cancelled() => {
            vlog!("Stream cancelled by user.");
            break;
          }
          chunk_result = stream.next() => match chunk_result {
            Some(result) => result.map_err(|e| RuntimeError::Provider(e.to_string()))?,
            None => break,
          },
        };

        if let Some(usage) = delta.usage {
          stream_usage = Some(usage);
        }

        if let Some(text) = delta.content {
          content_buffer.push_str(&text);
          sink.on_event(AppEvent::Token {
            text,
            reasoning: false,
          });
        }

        if let Some(text) = delta.reasoning {
          if self.config.general.show_reasoning {
            sink.on_event(AppEvent::Token {
              text: text.clone(),
              reasoning: true,
            });
          }
          reasoning_buffer.push_str(&text);
        }

        for tc_delta in delta.tool_calls {
          let entry =
            tool_call_builders
              .entry(tc_delta.index)
              .or_insert(ToolCall {
                id: String::new(),
                name: String::new(),
                arguments: String::new(),
              });
          if let Some(id) = tc_delta.id {
            entry.id = id;
          }
          if let Some(name) = tc_delta.name {
            entry.name = name;
          }
          if let Some(args) = tc_delta.arguments {
            entry.arguments.push_str(&args);
          }
        }
      }

      if let Some(usage) = stream_usage {
        self.last_usage = Some(usage);
      }

      if !content_buffer.is_empty() {
        assistant_content.push_str(&content_buffer);
      }

      if self.config.general.show_reasoning && !reasoning_buffer.is_empty() {
        self
          .session_manager
          .add_reasoning_message_to_current_session(&reasoning_buffer)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }
      if !content_buffer.is_empty() || tool_call_builders.is_empty() {
        self
          .session_manager
          .add_assistant_message_to_current_session(&content_buffer)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }

      if tool_call_builders.is_empty() {
        break;
      }

      let tool_calls: Vec<ToolCall> =
        tool_call_builders.into_values().collect();
      self.execute_tool_calls(&tool_calls, sink, cancel).await?;
    }

    return Ok(assistant_content);
  }

  async fn process_chat_non_streaming(
    &mut self,
    sink: &mut impl EventSink,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    let mut assistant_content = String::new();

    loop {
      if cancel.is_cancelled() {
        break;
      }
      let session = self
        .session_manager
        .get_current_session()
        .ok_or_else(|| RuntimeError::Session("No active session".into()))?;

      let messages = &session.build_chat_context();
      let tools = self
        .tool_manager
        .get_tools_specs(self.persona_manager.current().allowed_tools());

      let response = self
        .provider_manager
        .chat(messages, &tools)
        .await
        .map_err(|e| RuntimeError::Provider(e.to_string()))?;

      if let Some(usage) = response.usage {
        self.last_usage = Some(usage);
      }

      if self.config.general.show_reasoning
        && let Some(ref reasoning) = response.reasoning
      {
        sink.on_event(AppEvent::Token {
          text: reasoning.clone(),
          reasoning: true,
        });
      }

      if let Some(ref content) = response.content {
        sink.on_event(AppEvent::Token {
          text: content.clone(),
          reasoning: false,
        });
        assistant_content.push_str(content);
      }

      if self.config.general.show_reasoning
        && let Some(ref reasoning) = response.reasoning
        && !reasoning.is_empty()
      {
        self
          .session_manager
          .add_reasoning_message_to_current_session(reasoning)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }
      if let Some(ref content) = response.content {
        self
          .session_manager
          .add_assistant_message_to_current_session(content)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }

      if response.tool_calls.is_empty() {
        break;
      }

      self
        .execute_tool_calls(&response.tool_calls, sink, cancel)
        .await?;
    }

    return Ok(assistant_content);
  }

  async fn execute_tool_calls(
    &mut self,
    tool_calls: &[ToolCall],
    sink: &mut impl EventSink,
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
        sink.on_event(AppEvent::ToolCall {
          name: tool_call.name.clone(),
          arguments: tool_call.arguments.clone(),
        });
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
          if sink.confirm(&message) {
            None
          } else {
            Some(ToolRejection::UserDeclined)
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
          sink.on_event(AppEvent::ToolResult { result, error });
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
        sink.on_event(AppEvent::ToolResult {
          result: result_str,
          error,
        });
      }
    }
    return Ok(());
  }

  pub async fn harness_loop(
    &mut self,
    sink: &mut impl EventSink,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    loop {
      if self.shutdown.is_cancelled() {
        vlog!("Shutdown signal received, exiting harness loop...");
        break;
      }

      let input = sink.read_input().await?;
      if let Some(input) = input {
        let command = match AppCommand::parse(&input) {
          Ok(Some(command)) => Some(command),
          Ok(None) => None,
          Err(e) => {
            sink.on_event(AppEvent::Error {
              message: e.to_string(),
            });
            continue;
          }
        };

        if let Some(command) = command {
          match self.handle_command(command).await? {
            AppCommandResult::Exit => {
              sink.on_exit();
              break;
            }
            AppCommandResult::Help(slash_commands) => {
              sink.on_help(slash_commands);
            }
            AppCommandResult::Sessions { sessions, current } => {
              if let Some(session_id) = sink.on_sessions(sessions, current)?
                && let AppCommandResult::Session(session) = self
                  .handle_command(AppCommand::LoadSession { id: session_id })
                  .await?
              {
                sink.on_session_selection(session);
              }
            }
            AppCommandResult::Session(session) => {
              sink.on_session_selection(session);
            }
            AppCommandResult::Personas { personas, current } => {
              if let Some(name) = sink.on_personas(personas, current)? {
                self
                  .handle_command(AppCommand::SelectPersona {
                    name: name.clone(),
                  })
                  .await?;
                sink.on_persona_selection(name);
              }
            }
            AppCommandResult::Persona(name) => {
              sink.on_persona_selection(name);
            }
            AppCommandResult::Noop => {}
          }
          continue;
        }

        self.ensure_session_with_system_message().await?;

        self
          .session_manager
          .add_user_message_to_current_session(&input)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;

        sink.on_event(AppEvent::UserMessage { content: input });

        sink.on_event(AppEvent::Processing { processing: true });
        match self.process_chat(sink, cancel).await {
          Ok(_) => {
            sink.on_event(AppEvent::Processing { processing: false });
            sink.on_event(AppEvent::Usage {
              usage: self.last_usage(),
            });
          }
          Err(e) => {
            sink.on_event(AppEvent::Processing { processing: false });
            sink.on_event(AppEvent::Error {
              message: e.to_string(),
            });
            continue;
          }
        }
      } else {
        break;
      }
    }
    return Ok(String::new());
  }
}
