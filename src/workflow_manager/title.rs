use std::sync::Arc;

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::config::Config;
use crate::persona_manager::TITLES_PERSONA;
use crate::prompt::Prompt;
use crate::provider_manager::ProviderManager;
use crate::session_manager::Message;
use crate::vlog;

pub struct TitleParams {
  pub message: String,
}

pub struct TitleWorkflow;

impl TitleWorkflow {
  pub async fn execute(
    provider: Arc<ProviderManager>,
    config: Config,
    params: TitleParams,
  ) -> Result<String> {
    if params.message.is_empty() {
      return Ok(String::new());
    }

    let prompt = Prompt::new(&config, &TITLES_PERSONA);
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
        content: params.message,
        tool_call_id: None,
        tool_calls: None,
        timestamp: now,
      },
    ];

    let response = provider
      .chat(&messages, &[])
      .await
      .context("Failed to execute title workflow")?;

    let Some(content) = response.content else {
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
