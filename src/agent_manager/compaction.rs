use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::agent_manager::AgentManager;
use crate::app::App;
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::vlog;
use crate::workflow_manager::compaction::CompactionParams;

impl AgentManager {
  pub async fn maybe_compact_arc(
    app: Arc<Mutex<App>>,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    let mut app_locked = app.lock().await;
    return app_locked.agent_manager.maybe_compact(cancel).await;
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

    let params = CompactionParams {
      messages: session.messages.clone(),
      last_summary: session.summaries.last().cloned(),
    };

    let summary = match tokio::select! {
      biased;
      _ = cancel.cancelled() => {
        vlog!("Session compaction cancelled.");
        return Err(RuntimeError::Compaction("Cancelled".into()));
      }
      result = self.workflow_manager.compact(params) => result,
    } {
      Ok(summary) => summary,
      Err(e) => {
        vlog!("Failed to compact session: {e}");
        return Err(RuntimeError::Compaction(e.to_string()));
      }
    };

    let covers_message_id = session
      .messages
      .last()
      .ok_or(RuntimeError::Compaction("No messages to cover".into()))?
      .id;

    self
      .session_manager
      .add_summary_to_session(session.id, &summary, covers_message_id)
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;

    return Ok(summary);
  }
}
