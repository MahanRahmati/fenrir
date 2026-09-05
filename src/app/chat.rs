use std::collections::HashMap;

use futures::StreamExt;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

use crate::app::App;
use crate::app::AppEvent;
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::mentions::strip_mentions;
use crate::provider_manager::{ChatUsage, ToolCall};
use crate::vlog;
use crate::workflow_manager::title::TitleParams;

impl App {
  pub async fn process_chat(
    &mut self,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    if cancel.is_cancelled() {
      return Ok(String::new());
    }
    self.generate_title_from_workflow(cancel).await?;
    if self.config.general.stream {
      return self.process_chat_stream(tx, cancel).await;
    }
    return self.process_chat_non_streaming(tx, cancel).await;
  }

  async fn generate_title_from_workflow(
    &mut self,
    cancel: &CancellationToken,
  ) -> RuntimeResult<()> {
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

    let params = TitleParams {
      message: first_message,
    };

    let title = match tokio::select! {
      biased;
      _ = cancel.cancelled() => {
        vlog!("Session title generation cancelled.");
        return Ok(());
      }
      result = self.workflow_manager.generate_title(params) => result,
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

    self
      .session_manager
      .update_current_session_title(&title)
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;
    return Ok(());
  }

  async fn process_chat_stream(
    &mut self,
    tx: &Sender<AppEvent>,
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
      self.execute_tool_calls(&tool_calls, tx, cancel).await?;
    }

    return Ok(assistant_content);
  }

  async fn process_chat_non_streaming(
    &mut self,
    tx: &Sender<AppEvent>,
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
        .execute_tool_calls(&response.tool_calls, tx, cancel)
        .await?;
    }

    return Ok(assistant_content);
  }
}
