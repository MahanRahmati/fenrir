use std::sync::Arc;

use anyhow::{Context, Result, bail};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::agent_manager::AgentManager;
use crate::app::App;
use crate::vlog;
use crate::workflow_manager::compaction::CompactionParams;

impl AgentManager {
  pub async fn maybe_compact_arc(
    app: Arc<Mutex<App>>,
    cancel: &CancellationToken,
  ) -> Result<String> {
    let mut app_locked = app.lock().await;
    return app_locked.agent_manager.maybe_compact(cancel).await;
  }

  pub async fn maybe_compact(
    &mut self,
    cancel: &CancellationToken,
  ) -> Result<String> {
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
  ) -> Result<String> {
    let session = match self.session_manager.get_current_session() {
      Some(session) => session,
      None => {
        vlog!("Failed to compact session: no session selected");
        bail!("No session selected");
      }
    };
    if session.messages.is_empty() {
      vlog!("Failed to compact session: no messages in session");
      bail!("No messages in session");
    }

    let params = CompactionParams {
      messages: session.messages.clone(),
      last_summary: session.summaries.last().cloned(),
    };

    let summary = match tokio::select! {
      biased;
      _ = cancel.cancelled() => {
        vlog!("Session compaction cancelled.");
        bail!("Cancelled");
      }
      result = self.current.workflow_manager.compact(params) => result,
    } {
      Ok(summary) => summary,
      Err(e) => {
        vlog!("Failed to compact session: {e}");
        return Err(e.context("Failed to compact session"));
      }
    };

    let covers_message_id =
      session.messages.last().context("No messages to cover")?.id;

    self
      .session_manager
      .add_summary_to_session(session.id, &summary, covers_message_id)
      .await?;

    return Ok(summary);
  }
}
