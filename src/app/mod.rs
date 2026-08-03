mod cli;
pub mod errors;
mod tui;

use std::collections::HashMap;

use futures::StreamExt;

use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::config::Config;
use crate::database::DB;
use crate::prompt::Prompt;
use crate::provider_manager::ProviderManager;
use crate::provider_manager::ToolCall;
use crate::session_manager::SessionManager;
use crate::shutdown::Shutdown;
use crate::tool_manager::ToolManager;
use crate::tool_manager::tool::parse_tool_params;
use crate::vlog;

pub enum AppEvent {
  Token { text: String, reasoning: bool },
  Confirm(String),
  ToolCall { name: String, arguments: String },
  ToolResult { result: String, error: bool },
}

pub struct App {
  session_manager: SessionManager,
  provider_manager: ProviderManager,
  tool_manager: ToolManager,
  shutdown: Shutdown,
  config: Config,
}

impl App {
  pub fn new(config: Config, db: DB) -> Self {
    let session_manager = SessionManager::new(db);
    let tool_manager = ToolManager::new(&config);
    let provider_manager = ProviderManager::new(config.clone());
    let shutdown = Shutdown::new();
    return App {
      session_manager,
      provider_manager,
      tool_manager,
      shutdown,
      config,
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

    let prompt =
      Prompt::new(&self.config, &self.tool_manager.get_tools_specs());
    vlog!("Prompt: {}", prompt.get_system_prompt());
    self
      .session_manager
      .add_system_message_to_current_session(prompt.get_system_prompt())
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;

    return Ok(());
  }

  pub async fn process_chat<F>(
    &mut self,
    mut on_event: F,
  ) -> RuntimeResult<String>
  where
    F: FnMut(AppEvent) -> bool,
  {
    if self.config.general.stream {
      return self.process_chat_stream(&mut on_event).await;
    }
    return self.process_chat_non_streaming(&mut on_event).await;
  }

  async fn process_chat_stream<F>(
    &mut self,
    on_event: &mut F,
  ) -> RuntimeResult<String>
  where
    F: FnMut(AppEvent) -> bool,
  {
    let mut assistant_content = String::new();

    loop {
      let session = self
        .session_manager
        .get_current_session()
        .ok_or(RuntimeError::Session("No active session".into()))?;

      let messages = session.messages.clone();
      let tools = self.tool_manager.get_tools_specs();

      let mut stream = self
        .provider_manager
        .chat_stream(&messages, &tools)
        .await
        .map_err(|e| RuntimeError::Provider(e.to_string()))?;

      let mut content_buffer = String::new();
      let mut reasoning_buffer = String::new();
      let mut tool_call_builders: HashMap<usize, ToolCall> = HashMap::new();

      while let Some(chunk_result) = stream.next().await {
        let delta =
          chunk_result.map_err(|e| RuntimeError::Provider(e.to_string()))?;

        if let Some(ref text) = delta.content {
          on_event(AppEvent::Token {
            text: text.clone(),
            reasoning: false,
          });
          content_buffer.push_str(text);
        }

        if let Some(ref text) = delta.reasoning {
          if self.config.general.show_reasoning {
            on_event(AppEvent::Token {
              text: text.clone(),
              reasoning: true,
            });
          }
          reasoning_buffer.push_str(text);
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

      if !content_buffer.is_empty() {
        assistant_content.push_str(&content_buffer);
      }

      if tool_call_builders.is_empty() {
        if self.config.general.show_reasoning && !reasoning_buffer.is_empty() {
          self
            .session_manager
            .add_reasoning_message_to_current_session(&reasoning_buffer)
            .await
            .map_err(|e| RuntimeError::Session(e.to_string()))?;
        }
        self
          .session_manager
          .add_assistant_message_to_current_session(&content_buffer)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
        break;
      }

      let tool_calls: Vec<ToolCall> =
        tool_call_builders.into_values().collect();
      self.execute_tool_calls(&tool_calls, on_event).await?;
    }

    return Ok(assistant_content);
  }

  async fn process_chat_non_streaming<F>(
    &mut self,
    on_event: &mut F,
  ) -> RuntimeResult<String>
  where
    F: FnMut(AppEvent) -> bool,
  {
    let mut assistant_content = String::new();

    loop {
      let session = self
        .session_manager
        .get_current_session()
        .ok_or_else(|| RuntimeError::Session("No active session".into()))?;

      let messages = session.messages.clone();
      let tools = self.tool_manager.get_tools_specs();

      let response = self
        .provider_manager
        .chat(&messages, &tools)
        .await
        .map_err(|e| RuntimeError::Provider(e.to_string()))?;

      if self.config.general.show_reasoning
        && let Some(ref reasoning) = response.reasoning
      {
        on_event(AppEvent::Token {
          text: reasoning.clone(),
          reasoning: true,
        });
      }

      if let Some(ref content) = response.content {
        on_event(AppEvent::Token {
          text: content.clone(),
          reasoning: false,
        });
        assistant_content.push_str(content);
      }

      if response.tool_calls.is_empty() {
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
        break;
      }

      self
        .execute_tool_calls(&response.tool_calls, on_event)
        .await?;
    }

    return Ok(assistant_content);
  }

  async fn execute_tool_calls<F: FnMut(AppEvent) -> bool>(
    &mut self,
    tool_calls: &[ToolCall],
    on_event: &mut F,
  ) -> RuntimeResult<()> {
    for tool_call in tool_calls {
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
        on_event(AppEvent::ToolCall {
          name: tool_call.name.clone(),
          arguments: tool_call.arguments.clone(),
        });
      }

      let params = parse_tool_params(&tool_call.arguments);

      let mut confirm =
        |msg: &str| on_event(AppEvent::Confirm(msg.to_string()));
      if !self.tool_manager.confirm_if_needed(
        &tool_call.name,
        &params,
        &mut confirm,
      ) {
        let result = "Tool execution cancelled by user".to_string();
        self
          .session_manager
          .add_tool_result_to_current_session(&tool_call.id, &result)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
        if self.config.general.show_tools {
          on_event(AppEvent::ToolResult {
            result,
            error: false,
          });
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
          (format!("Tool error: {}", e), true)
        }
      };

      self
        .session_manager
        .add_tool_result_to_current_session(&tool_call.id, &result_str)
        .await
        .map_err(|e| RuntimeError::Session(e.to_string()))?;

      if self.config.general.show_tools {
        on_event(AppEvent::ToolResult {
          result: result_str,
          error,
        });
      }
    }
    return Ok(());
  }
}
