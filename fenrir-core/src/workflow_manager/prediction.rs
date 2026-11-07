use anyhow::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::engine::protocol::Correlation;
use crate::prompt::PromptExecutor;
use crate::prompt::prediction::{build, truncate};
use crate::provider_manager::{Model, ProviderManager};
use crate::session_manager::Message;
use crate::vlog;

const MAX_PREDICTION_CHARS: usize = 300;

pub struct PredictionParams {
  pub messages: Vec<Message>,
  pub summary: Option<String>,
}

pub struct PredictionWorkflow;

impl PredictionWorkflow {
  pub async fn execute(
    provider: &ProviderManager,
    config: &Config,
    model: &Model,
    params: PredictionParams,
    cwd: &str,
  ) -> Result<String> {
    let spec = build(config, cwd, model, params.messages, params.summary);
    if spec.messages.is_empty() {
      return Ok(String::new());
    }
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
      return Ok(String::new());
    };

    let prediction = content
      .trim()
      .trim_matches(|c| c == '"' || c == '\'' || c == '`')
      .trim()
      .to_string();
    if prediction.is_empty() {
      return Ok(String::new());
    }

    let prediction = truncate(&prediction, MAX_PREDICTION_CHARS);
    vlog!("Predicted next prompt: {prediction}");
    return Ok(prediction);
  }
}
