use anyhow::{Context, Result};
use uuid::Uuid;

use crate::config::Config;
use crate::persona_manager::PREDICTION_PERSONA;
use crate::prompt::Prompt;
use crate::provider_manager::{Model, ProviderManager};
use crate::session_manager::Message;
use crate::vlog;

const MAX_MESSAGE_CHARS: usize = 1000;
const MAX_SUMMARY_CHARS: usize = 4000;
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
    cwd: &String,
  ) -> Result<String> {
    let prompt = Prompt::new(config, &PREDICTION_PERSONA, cwd);
    let mut system_prompt = prompt.get_system_prompt().to_string();

    if let Some(summary) = params
      .summary
      .as_deref()
      .map(str::trim)
      .filter(|summary| !summary.is_empty())
    {
      system_prompt.push_str("\n\nEarlier conversation summary:\n");
      system_prompt.push_str(&truncate(summary, MAX_SUMMARY_CHARS));
    }

    let mut transcript = String::new();
    for message in &params.messages {
      if message.role != "user" && message.role != "assistant" {
        continue;
      }
      let content = message.content.trim();
      if content.is_empty() {
        continue;
      }
      let label = if message.role == "user" {
        "User"
      } else {
        "Assistant"
      };
      if !transcript.is_empty() {
        transcript.push_str("\n\n");
      }
      transcript.push_str(&format!(
        "[{label}]: {}",
        truncate(content, MAX_MESSAGE_CHARS)
      ));
    }

    if transcript.is_empty() {
      return Ok(String::new());
    }

    let user_content = format!(
      "Recent conversation:\n{transcript}\n\nPredict the user's next message. Output only the message text."
    );

    let now = chrono::Utc::now().timestamp();
    let messages = vec![
      Message {
        id: Uuid::now_v7(),
        msg_type: None,
        role: "system".to_string(),
        content: system_prompt,
        tool_call_id: None,
        tool_calls: None,
        timestamp: now,
      },
      Message {
        id: Uuid::now_v7(),
        msg_type: None,
        role: "user".to_string(),
        content: user_content,
        tool_call_id: None,
        tool_calls: None,
        timestamp: now,
      },
    ];

    let response = provider
      .chat(model, &messages, &[])
      .await
      .context("Failed to execute prediction workflow")?;

    let Some(content) = response.content else {
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

fn truncate(content: &str, max_chars: usize) -> String {
  if content.chars().count() <= max_chars {
    return content.to_string();
  }
  let mut truncated: String =
    content.chars().take(max_chars.saturating_sub(1)).collect();
  truncated.push('…');
  return truncated;
}
