use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::app::AppEvent;
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::config::Config;
use crate::mentions::strip_mentions;
use crate::persona_manager::PersonaManager;
use crate::provider_manager::{ChatUsage, ProviderManager, ToolCall};
use crate::session_manager::SessionManager;
use crate::tool_manager::ToolApproval;
use crate::tool_manager::ToolManager;
use crate::tool_manager::ToolRejection;
use crate::tool_manager::tool::parse_tool_params;
use crate::vlog;
use crate::workflow_manager::WorkflowManager;
use crate::workflow_manager::title::TitleParams;

pub struct Agent {
  pub config: Config,
  pub tool_manager: ToolManager,
  pub persona_manager: PersonaManager,
  pub provider_manager: Arc<ProviderManager>,
  pub last_usage: Option<ChatUsage>,
}

impl Agent {
  pub fn new(config: Config, provider_manager: Arc<ProviderManager>) -> Self {
    let tool_manager = ToolManager::new(&config);
    let persona_manager = PersonaManager::new();
    return Agent {
      config,
      tool_manager,
      persona_manager,
      provider_manager,
      last_usage: None,
    };
  }

  pub async fn process_chat(
    &mut self,
    session_manager: &mut SessionManager,
    workflow_manager: &WorkflowManager,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    if cancel.is_cancelled() {
      return Ok(String::new());
    }
    self
      .generate_title_from_workflow(session_manager, workflow_manager, cancel)
      .await?;
    return self.run_chat_loop(session_manager, tx, cancel).await;
  }

  pub async fn run_chat_loop(
    &mut self,
    session_manager: &mut SessionManager,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    if cancel.is_cancelled() {
      return Ok(String::new());
    }
    if self.config.general.stream {
      return self.process_chat_stream(session_manager, tx, cancel).await;
    }
    return self
      .process_chat_non_streaming(session_manager, tx, cancel)
      .await;
  }

  async fn generate_title_from_workflow(
    &mut self,
    session_manager: &mut SessionManager,
    workflow_manager: &WorkflowManager,
    cancel: &CancellationToken,
  ) -> RuntimeResult<()> {
    let session = match session_manager.get_current_session() {
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

    let params = TitleParams {
      message: first_message,
    };

    let title = match tokio::select! {
      biased;
      _ = cancel.cancelled() => {
        vlog!("Session title generation cancelled.");
        return Ok(());
      }
      result = workflow_manager.generate_title(params) => result,
    } {
      Ok(title) => title,
      Err(e) => {
        vlog!("Failed to generate session title: {e}");
        return Ok(());
      }
    };

    if title.is_empty() {
      return Ok(());
    }

    session_manager
      .update_session_title(session.id, &title)
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;
    return Ok(());
  }

  async fn process_chat_stream(
    &mut self,
    session_manager: &mut SessionManager,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    let mut assistant_content = String::new();

    loop {
      let session = session_manager
        .get_current_session()
        .ok_or(RuntimeError::Session("No active session".into()))?;
      let session_id = session.id;

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
          tx.send(AppEvent::Token {
            text,
            reasoning: false,
          })
          .await
          .ok();
        }

        if let Some(text) = delta.reasoning {
          if self.config.general.show_reasoning {
            tx.send(AppEvent::Token {
              text: text.clone(),
              reasoning: true,
            })
            .await
            .ok();
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
        session_manager
          .add_reasoning_message_to_session(session_id, &reasoning_buffer)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }
      if !content_buffer.is_empty() || tool_call_builders.is_empty() {
        session_manager
          .add_assistant_message_to_session(session_id, &content_buffer)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }

      if tool_call_builders.is_empty() {
        break;
      }

      let tool_calls: Vec<ToolCall> =
        tool_call_builders.into_values().collect();
      self
        .execute_tool_calls(
          session_manager,
          session_id,
          &tool_calls,
          tx,
          cancel,
        )
        .await?;
    }

    return Ok(assistant_content);
  }

  async fn process_chat_non_streaming(
    &mut self,
    session_manager: &mut SessionManager,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    let mut assistant_content = String::new();

    loop {
      if cancel.is_cancelled() {
        break;
      }
      let session = session_manager
        .get_current_session()
        .ok_or(RuntimeError::Session("No active session".into()))?;
      let session_id = session.id;

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
        tx.send(AppEvent::Token {
          text: reasoning.clone(),
          reasoning: true,
        })
        .await
        .ok();
      }

      if let Some(ref content) = response.content {
        tx.send(AppEvent::Token {
          text: content.clone(),
          reasoning: false,
        })
        .await
        .ok();
        assistant_content.push_str(content);
      }

      if self.config.general.show_reasoning
        && let Some(ref reasoning) = response.reasoning
        && !reasoning.is_empty()
      {
        session_manager
          .add_reasoning_message_to_session(session_id, reasoning)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }
      if let Some(ref content) = response.content {
        session_manager
          .add_assistant_message_to_session(session_id, content)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }

      if response.tool_calls.is_empty() {
        break;
      }

      self
        .execute_tool_calls(
          session_manager,
          session_id,
          &response.tool_calls,
          tx,
          cancel,
        )
        .await?;
    }

    return Ok(assistant_content);
  }

  async fn execute_tool_calls(
    &mut self,
    session_manager: &mut SessionManager,
    session_id: Uuid,
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

      session_manager
        .add_tool_call_to_session(
          session_id,
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
        session_manager
          .add_tool_result_to_session(session_id, &tool_call.id, &result)
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

      session_manager
        .add_tool_result_to_session(session_id, &tool_call.id, &result_str)
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
