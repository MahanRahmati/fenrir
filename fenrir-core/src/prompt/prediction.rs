use uuid::Uuid;

use crate::config::Config;
use crate::persona_manager::PREDICTION_PERSONA;
use crate::prompt::{PromptKind, PromptSpec, system};
use crate::provider_manager::Model;
use crate::session_manager::Message;

const MAX_MESSAGE_CHARS: usize = 1000;
const MAX_SUMMARY_CHARS: usize = 4000;

pub fn build(
  config: &Config,
  cwd: &str,
  model: &Model,
  messages: Vec<Message>,
  summary: Option<String>,
) -> PromptSpec {
  let mut system_prompt = system::build(config, &PREDICTION_PERSONA, cwd);

  if let Some(summary) = summary
    .as_deref()
    .map(str::trim)
    .filter(|summary| !summary.is_empty())
  {
    system_prompt.push_str("\n\nEarlier conversation summary:\n");
    system_prompt.push_str(&truncate(summary, MAX_SUMMARY_CHARS));
  }

  let mut transcript = String::new();
  for message in &messages {
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
    return PromptSpec {
      kind: PromptKind::Prediction,
      model: model.clone(),
      system: system_prompt,
      messages: Vec::new(),
      tools: Vec::new(),
      stream: false,
      emit_tokens: false,
    };
  }

  let user_content = format!(
    "Recent conversation:\n{transcript}\n\nPredict the user's next message. Output only the message text."
  );

  let now = chrono::Utc::now().timestamp();
  let user_message = Message {
    id: Uuid::now_v7(),
    msg_type: None,
    role: "user".to_string(),
    content: user_content,
    tool_call_id: None,
    tool_calls: None,
    timestamp: now,
  };

  return PromptSpec {
    kind: PromptKind::Prediction,
    model: model.clone(),
    system: system_prompt,
    messages: vec![user_message],
    tools: Vec::new(),
    stream: false,
    emit_tokens: false,
  };
}

pub fn truncate(content: &str, max_chars: usize) -> String {
  if content.chars().count() <= max_chars {
    return content.to_string();
  }
  let mut truncated: String =
    content.chars().take(max_chars.saturating_sub(1)).collect();
  truncated.push('…');
  return truncated;
}
