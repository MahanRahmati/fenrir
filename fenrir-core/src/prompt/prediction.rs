use anyhow::Result;
use tokio::sync::mpsc::{self, Sender};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::config::secrets::Secrets;
use crate::engine::protocol::{Correlation, ServerEvent, SessionEvent};
use crate::persona_manager::PREDICTION_PERSONA;
use crate::prompt::{PromptExecutor, PromptKind, PromptSpec, system};
use crate::provider_manager::{Model, ProviderManager};
use crate::session_manager::Message;
use crate::session_manager::session::Session;
use crate::vlog;

const MAX_MESSAGE_CHARS: usize = 1000;
const MAX_SUMMARY_CHARS: usize = 4000;
const MAX_CONTEXT_MESSAGES: usize = 10;
const MAX_PREDICTION_CHARS: usize = 300;

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

pub fn spawn(
  session: &Session,
  config: &Config,
  secrets: &Secrets,
  events: Sender<SessionEvent>,
) {
  let session_id = session.id;
  let model = session.model.clone();
  let messages = if session.is_subagent() {
    Vec::new()
  } else {
    prediction_messages(session)
  };
  let summary = session
    .summaries
    .last()
    .map(|summary| summary.content.clone());
  let cwd = session.cwd.clone();
  let config = config.clone();
  let secrets = secrets.clone();
  tokio::spawn(async move {
    let providers = ProviderManager::new(config.clone(), secrets);
    let model = providers.small_model().or(model);
    let text = match model {
      Some(model) if !messages.is_empty() => {
        match run(&providers, &config, &model, &cwd, messages, summary).await {
          Ok(text) => text,
          Err(e) => {
            vlog!("Failed to predict next prompt: {e}");
            String::new()
          }
        }
      }
      _ => String::new(),
    };
    let _ = events
      .send(SessionEvent::Event(ServerEvent::Prediction {
        session_id,
        text,
      }))
      .await;
  });
}

async fn run(
  providers: &ProviderManager,
  config: &Config,
  model: &Model,
  cwd: &str,
  messages: Vec<Message>,
  summary: Option<String>,
) -> Result<String> {
  let spec = build(config, cwd, model, messages, summary);
  if spec.messages.is_empty() {
    return Ok(String::new());
  }

  let (events, _events_rx) = mpsc::channel(1);
  let correlation = Correlation {
    session_id: Uuid::nil(),
    turn_id: Uuid::nil(),
  };
  let cancel = CancellationToken::new();
  let output = PromptExecutor::new(providers, config)
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

fn prediction_messages(session: &Session) -> Vec<Message> {
  let mut messages: Vec<Message> = session
    .build_chat_context()
    .into_iter()
    .filter(|message| {
      (message.role == "user" || message.role == "assistant")
        && message.tool_calls.is_none()
        && !message.content.trim().is_empty()
    })
    .collect();
  if messages.len() > MAX_CONTEXT_MESSAGES {
    messages = messages.split_off(messages.len() - MAX_CONTEXT_MESSAGES);
  }
  return messages;
}
