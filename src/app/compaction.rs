use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::app::App;
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::persona_manager::COMPACTION_PERSONA;
use crate::prompt::Prompt;
use crate::session_manager::Message;
use crate::vlog;

impl App {
  pub async fn maybe_compact_arc(
    app: Arc<Mutex<App>>,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    let mut app_locked = app.lock().await;
    return app_locked.maybe_compact(cancel).await;
  }

  pub async fn maybe_compact(
    &mut self,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    if let Some(threshold) = self.config.compaction.threshold
      && let Some(last_usage) = self.last_usage()
      && last_usage.prompt_tokens as usize >= (threshold as f64 * 0.8) as usize
    {
      let summary = self.compact(cancel).await?;
      return Ok(summary);
    }
    return Ok(String::new());
  }

  pub async fn compact(
    &mut self,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    let session = match self.session_manager.get_current_session() {
      Some(session) => session,
      None => {
        vlog!("Failed to compact session: no session selected");
        return Err(RuntimeError::Compaction("No session selected".into()));
      }
    };
    if session.messages.is_empty() {
      vlog!("Failed to compact session: no messages in session");
      return Err(RuntimeError::Compaction("No messages in session".into()));
    }

    let mut messages = Vec::new();

    let prompt = Prompt::new(&self.config, &COMPACTION_PERSONA);
    let now = chrono::Utc::now().timestamp();

    let system_prompt_message = Message {
      id: Uuid::now_v7(),
      msg_type: None,
      role: "system".to_string(),
      content: prompt.get_system_prompt().to_string(),
      tool_call_id: None,
      tool_calls: None,
      timestamp: now,
    };

    messages.push(system_prompt_message);

    if let Some(latest) = session.summaries.last() {
      let first_user_message =
        session.messages.iter().find(|m| m.role == "user");
      if let Some(first_user_message) = first_user_message {
        let serialized = first_user_message.serialize_for_compaction(
          self.config.compaction.tool_result_max_chars,
        );
        if !serialized.is_empty() {
          messages.push(Message {
            id: Uuid::now_v7(),
            msg_type: None,
            role: first_user_message.role.clone(),
            content: serialized,
            tool_call_id: first_user_message.tool_call_id.clone(),
            tool_calls: first_user_message.tool_calls.clone(),
            timestamp: first_user_message.timestamp,
          });
        }
      }

      let summary = Message {
        id: Uuid::now_v7(),
        msg_type: None,
        role: "system".to_string(),
        content: latest.content.clone(),
        tool_call_id: None,
        tool_calls: None,
        timestamp: latest.created_at,
      };
      messages.push(summary);

      for message in &session.messages {
        if message.id > latest.covers_message_id {
          let serialized = message.serialize_for_compaction(
            self.config.compaction.tool_result_max_chars,
          );
          if !serialized.is_empty() {
            messages.push(Message {
              id: Uuid::now_v7(),
              msg_type: None,
              role: message.role.clone(),
              content: serialized,
              tool_call_id: message.tool_call_id.clone(),
              tool_calls: message.tool_calls.clone(),
              timestamp: message.timestamp,
            });
          }
        }
      }
    } else {
      for message in session.messages.iter() {
        let serialized = message.serialize_for_compaction(
          self.config.compaction.tool_result_max_chars,
        );
        if !serialized.is_empty() {
          messages.push(Message {
            id: Uuid::now_v7(),
            msg_type: None,
            role: message.role.clone(),
            content: serialized,
            tool_call_id: message.tool_call_id.clone(),
            tool_calls: message.tool_calls.clone(),
            timestamp: message.timestamp,
          });
        }
      }
    }

    let response = match tokio::select! {
      biased;
      _ = cancel.cancelled() => {
        vlog!("Session compaction cancelled.");
        return Err(RuntimeError::Compaction("Cancelled".into()));
      }
      result = self.provider_manager.chat(&messages, &[]) => result,
    } {
      Ok(response) => response,
      Err(e) => {
        vlog!("Failed to compact session: {e}");
        return Err(RuntimeError::Compaction(e.to_string()));
      }
    };
    let Some(content) = response.content else {
      vlog!("Failed to compact session: no content in response");
      return Err(RuntimeError::Compaction("No content in response".into()));
    };
    let content = content.trim();
    if content.is_empty() {
      vlog!("Failed to compact session: empty content");
      return Err(RuntimeError::Compaction("Empty content".into()));
    }
    vlog!("Compacted session: {content}");

    let covers_message_id = session
      .messages
      .last()
      .ok_or(RuntimeError::Compaction("No messages to cover".into()))?
      .id;

    self
      .session_manager
      .add_summary_to_current_session(content, covers_message_id)
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;

    return Ok(content.to_string());
  }
}
