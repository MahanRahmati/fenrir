use std::fmt::Write;

use jiff::Timestamp;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::engine::protocol::{Correlation, SessionEvent};
use crate::persona_manager::CONTINUATION_PERSONA;
use crate::prompt::{PromptExecutor, PromptKind, PromptSpec};
use crate::provider_manager::{Model, ProviderManager};
use crate::session_manager::Message;
use crate::session_manager::session::Session;
use crate::vlog;

const MAX_MESSAGE_CHARS: usize = 1000;
const MAX_REQUEST_CHARS: usize = 2000;
const MAX_SUMMARY_CHARS: usize = 4000;
const MAX_CONTEXT_MESSAGES: usize = 20;

pub const CONTINUE_INSTRUCTION: &str = "Continue.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
  Done,
  Continue,
}

impl Verdict {
  pub fn parse(content: &str) -> Self {
    let word = content
      .split(|c: char| !c.is_alphanumeric())
      .find(|word| !word.is_empty())
      .unwrap_or_default();
    return if word.eq_ignore_ascii_case("CONTINUE") {
      Self::Continue
    } else {
      Self::Done
    };
  }
}

fn system_prompt() -> String {
  let guidelines = CONTINUATION_PERSONA
    .instructions()
    .iter()
    .map(|instruction| format!("- {instruction}"))
    .collect::<Vec<String>>()
    .join("\n");
  return format!(
    "{}\n\nGuidelines:\n{guidelines}",
    CONTINUATION_PERSONA.identity()
  );
}

fn transcript(messages: &[Message]) -> String {
  let mut entries: Vec<String> =
    messages.iter().filter_map(format_message).collect();
  if entries.len() > MAX_CONTEXT_MESSAGES {
    entries = entries.split_off(entries.len() - MAX_CONTEXT_MESSAGES);
  }
  return entries.join("\n\n");
}

fn is_nudge(content: &str) -> bool {
  let word = content.trim().trim_end_matches(['.', '!']).trim();
  return word.eq_ignore_ascii_case("continue");
}

fn request(messages: &[Message]) -> Option<String> {
  return messages
    .iter()
    .rev()
    .filter(|message| message.role == "user")
    .map(|message| message.content.trim())
    .find(|content| !content.is_empty() && !is_nudge(content))
    .map(|content| truncate(content, MAX_REQUEST_CHARS));
}

fn format_message(message: &Message) -> Option<String> {
  let label = match message.role.as_str() {
    "user" => "User",
    "assistant" => "Assistant",
    "tool" => "Tool result",
    _ => return None,
  };

  let mut body = String::new();
  let content = message.content.trim();
  if !content.is_empty() {
    body.push_str(&truncate(content, MAX_MESSAGE_CHARS));
  }

  if let Some(calls) = message.tool_calls.as_deref() {
    let names = calls
      .iter()
      .map(|call| call.function.name.as_str())
      .collect::<Vec<&str>>()
      .join(", ");
    if !names.is_empty() {
      if !body.is_empty() {
        body.push('\n');
      }
      let _ = write!(body, "Tool calls: {names}");
    }
  }

  if body.is_empty() {
    return None;
  }
  return Some(format!("[{label}]: {body}"));
}

pub fn build(
  cwd: &str,
  model: &Model,
  messages: &[Message],
  summary: Option<String>,
) -> PromptSpec {
  let mut system = system_prompt();
  if let Some(summary) = summary
    .as_deref()
    .map(str::trim)
    .filter(|summary| !summary.is_empty())
  {
    system.push_str("\n\nEarlier conversation summary:\n");
    system.push_str(&truncate(summary, MAX_SUMMARY_CHARS));
  }

  let transcript = transcript(messages);
  if transcript.is_empty() {
    return PromptSpec {
      kind: PromptKind::Continuation,
      model: model.clone(),
      system,
      messages: Vec::new(),
      tools: Vec::new(),
      stream: false,
      emit_tokens: false,
      subagent: None,
      cwd: cwd.to_string(),
    };
  }

  let mut content = String::new();
  if let Some(request) = request(messages) {
    let _ = write!(content, "User request:\n{request}\n\n");
  }
  let _ = write!(
    content,
    "Recent conversation:\n{transcript}\n\nDid the assistant finish the user's request? Answer DONE or CONTINUE."
  );

  let user_message = Message {
    id: Uuid::now_v7(),
    msg_type: None,
    role: "user".to_string(),
    content,
    tool_call_id: None,
    tool_calls: None,
    timestamp: Timestamp::now().as_second(),
  };

  return PromptSpec {
    kind: PromptKind::Continuation,
    model: model.clone(),
    system,
    messages: vec![user_message],
    tools: Vec::new(),
    stream: false,
    emit_tokens: false,
    subagent: None,
    cwd: cwd.to_string(),
  };
}

pub async fn should_continue(
  session: &Session,
  config: &Config,
  provider_manager: &ProviderManager,
  correlation: Correlation,
  events: &Sender<SessionEvent>,
  cancel: &CancellationToken,
) -> bool {
  if cancel.is_cancelled() {
    return false;
  }
  let Some(model) = provider_manager
    .small_model()
    .or_else(|| session.model.clone())
  else {
    return false;
  };
  let summary = session
    .summaries
    .last()
    .map(|summary| summary.content.clone());
  let messages = session.build_chat_context();
  let spec = build(&session.cwd, &model, &messages, summary);
  if spec.messages.is_empty() {
    return false;
  }

  let output = PromptExecutor::new(provider_manager, config)
    .once(spec, None, correlation, events, cancel)
    .await;
  let output = match output {
    Ok(output) => output,
    Err(error) => {
      if !cancel.is_cancelled() {
        vlog!("Continuation check failed: {error}");
      }
      return false;
    }
  };
  let Some(content) = output.content else {
    return false;
  };
  let verdict = Verdict::parse(&content);
  vlog!("Continuation check: {verdict:?}");
  return verdict == Verdict::Continue;
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
