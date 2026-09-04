use uuid::Uuid;

use tokio_util::sync::CancellationToken;

use crate::app::App;
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::mentions::strip_mentions;
use crate::persona_manager::TITLES_PERSONA;
use crate::prompt::Prompt;
use crate::session_manager::Message;
use crate::vlog;

impl App {
  pub async fn generate_session_title(
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

    let response = match tokio::select! {
      biased;
      _ = cancel.cancelled() => {
        vlog!("Session title generation cancelled.");
        return Ok(());
      }
      result = self.provider_manager.chat(&messages, &[]) => result,
    } {
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
}
