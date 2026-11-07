use anyhow::{Result, bail};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::engine::protocol::Correlation;
use crate::prompt::PromptExecutor;
use crate::prompt::compaction::build;
use crate::provider_manager::{Model, ProviderManager};
use crate::session_manager::{Message, Summary};
use crate::vlog;

pub struct CompactionParams {
  pub messages: Vec<Message>,
  pub last_summary: Option<Summary>,
}

pub struct CompactionWorkflow;

impl CompactionWorkflow {
  pub async fn execute(
    provider: &ProviderManager,
    config: &Config,
    model: &Model,
    params: CompactionParams,
    cwd: &str,
  ) -> Result<String> {
    let spec = build(config, cwd, model, params.messages, params.last_summary);
    let (events, _events_rx) = mpsc::channel(1);
    let correlation = Correlation {
      session_id: Uuid::nil(),
      turn_id: Uuid::nil(),
    };
    let cancel = CancellationToken::new();
    let output = PromptExecutor::new(provider, config)
      .once(spec, correlation, &events, &cancel)
      .await?;

    let Some(content) = output.content else {
      bail!("Compaction workflow returned no content");
    };

    let content = content.trim().to_string();
    if content.is_empty() {
      bail!("Compaction workflow returned empty content");
    }

    vlog!("Compacted session: {content}");
    return Ok(content);
  }
}
