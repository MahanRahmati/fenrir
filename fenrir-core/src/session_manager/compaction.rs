use anyhow::{Context, Result, bail};
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::provider_manager::ProviderManager;
use crate::session_manager::SessionRepository;
use crate::session_manager::session::Session;
use crate::vlog;
use crate::workflow_manager::compaction::CompactionParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionMode {
  Auto,
  Forced,
}

impl Session {
  pub async fn compact(
    &mut self,
    config: &Config,
    providers: &ProviderManager,
    repository: &SessionRepository,
    mode: CompactionMode,
    cancel: &CancellationToken,
  ) -> Result<String> {
    if mode == CompactionMode::Auto
      && !self.reached_compaction_threshold(config)
    {
      return Ok(String::new());
    }

    if self.messages.is_empty() {
      vlog!("Failed to compact session: no messages in session");
      bail!("No messages in session");
    }
    let Some(model) = self.model.clone() else {
      bail!("No model selected");
    };

    let params = CompactionParams {
      messages: self.messages.clone(),
      last_summary: self.summaries.last().cloned(),
    };

    let summary = match tokio::select! {
      biased;
      _ = cancel.cancelled() => {
        vlog!("Session compaction cancelled.");
        bail!("Cancelled");
      }
      result = self
        .workflow_manager
        .compact(providers, &model, params) => result,
    } {
      Ok(summary) => summary,
      Err(e) => {
        vlog!("Failed to compact session: {e}");
        return Err(e.context("Failed to compact session"));
      }
    };

    let covers_message_id =
      self.messages.last().context("No messages to cover")?.id;

    self.add_summary(&summary, covers_message_id);
    let summary = self.summaries.last().context("No summary to persist")?;
    repository.insert_summary(self.id, summary).await?;

    return Ok(summary.content.clone());
  }

  fn reached_compaction_threshold(&self, config: &Config) -> bool {
    let Some(threshold) = config.compaction.threshold else {
      return false;
    };
    let Some(last_usage) = self.last_usage else {
      return false;
    };
    return last_usage.prompt_tokens as usize
      >= (threshold as f64 * 0.8) as usize;
  }
}
