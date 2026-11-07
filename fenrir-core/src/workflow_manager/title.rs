use anyhow::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::engine::protocol::Correlation;
use crate::prompt::{PromptExecutor, title};
use crate::provider_manager::{Model, ProviderManager};
use crate::session_manager::Message;
use crate::vlog;

pub struct TitleParams {
  pub message: String,
}

pub struct TitleWorkflow;

impl TitleWorkflow {
  pub async fn execute(
    provider: &ProviderManager,
    config: &Config,
    model: &Model,
    params: TitleParams,
    cwd: &str,
  ) -> Result<String> {
    if params.message.is_empty() {
      return Ok(String::new());
    }

    let now = chrono::Utc::now().timestamp();
    let message = Message {
      id: Uuid::now_v7(),
      msg_type: None,
      role: "user".to_string(),
      content: params.message,
      tool_call_id: None,
      tool_calls: None,
      timestamp: now,
    };
    let spec = title::build(config, cwd, model, message);
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

    let title = content
      .trim()
      .trim_matches(|c| c == '"' || c == '\'' || c == '`')
      .to_string();

    if title.is_empty() {
      return Ok(String::new());
    }

    vlog!("Generated session title: {title}");
    return Ok(title);
  }
}
