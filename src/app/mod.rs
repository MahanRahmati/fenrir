mod cli;
pub mod errors;
mod tui;

use std::collections::HashMap;

use futures::StreamExt;

use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::config::Config;
use crate::database::DB;
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

    let prompt = Prompt::new(
      &self.config,
      &self.tool_manager.get_tools_specs(),
      self.persona_manager.current(),
    );
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

  pub async fn process_chat<F>(
    &mut self,
    mut on_event: F,
  ) -> RuntimeResult<String>
  where
    F: FnMut(AppEvent) -> bool,
  {
    self.generate_session_title().await?;
    if self.config.general.stream {
      return self.process_chat_stream(&mut on_event).await;
    }
    return self.process_chat_non_streaming(&mut on_event).await;
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
      .unwrap_or_default();
    if first_message.is_empty() {
      return Ok(());
    }

    let prompt = Prompt::new(&self.config, &[], &TITLES_PERSONA);
    let now = chrono::Utc::now().timestamp();
    let messages = vec![
      Message {
        msg_type: None,
        role: "system".to_string(),
        content: prompt.get_system_prompt().to_string(),
        tool_call_id: None,
        tool_calls: None,
        timestamp: now,
      },
      Message {
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

      let messages = &session.messages;
      let tools = self.tool_manager.get_tools_specs();

      let mut stream = self
        .provider_manager
        .chat_stream(messages, &tools)
        .await
        .map_err(|e| RuntimeError::Provider(e.to_string()))?;

      let mut content_buffer = String::new();
      let mut reasoning_buffer = String::new();
      let mut stream_usage: Option<ChatUsage> = None;
      let mut tool_call_builders: HashMap<usize, ToolCall> = HashMap::new();

      while let Some(chunk_result) = stream.next().await {
        let delta =
          chunk_result.map_err(|e| RuntimeError::Provider(e.to_string()))?;

        if let Some(usage) = delta.usage {
          stream_usage = Some(usage);
        }

        if let Some(text) = delta.content {
          content_buffer.push_str(&text);
          on_event(AppEvent::Token {
            text,
            reasoning: false,
          });
        }

        if let Some(text) = delta.reasoning {
          if self.config.general.show_reasoning {
            on_event(AppEvent::Token {
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

      let messages = &session.messages;
      let tools = self.tool_manager.get_tools_specs();

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
          (format!("Tool error: {e}"), true)
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
