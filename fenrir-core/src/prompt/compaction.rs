use anyhow::{Context, Result, bail};
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::engine::protocol::{Correlation, ServerEvent, SessionEvent};
use crate::engine::repo_actor::RepoHandle;
use crate::persona_manager::COMPACTION_PERSONA;
use crate::prompt::{PromptExecutor, PromptKind, PromptSpec, system};
use crate::provider_manager::{Model, ProviderManager};
use crate::session_manager::session::Session;
use crate::session_manager::{Message, Summary};
use crate::vlog;

const SUMMARY_INSTRUCTION: &str = "Write the structured summary of the conversation above now, using the exact format from the system prompt. Do not continue the conversation.";

pub fn build(
  config: &Config,
  cwd: &str,
  model: &Model,
  messages: Vec<Message>,
  last_summary: Option<Summary>,
) -> PromptSpec {
  let mut system_prompt = system::build(config, &COMPACTION_PERSONA, cwd);
  if let Some(ref summary) = last_summary {
    system_prompt.push_str("\n\nPrevious summary:\n");
    system_prompt.push_str(&summary.content);
  }

  let max_chars = config.compaction.tool_result_max_chars;
  let serialized: Vec<Message> = messages
    .iter()
    .filter(|message| message.role != "system")
    .filter_map(|message| {
      let content = message.serialize_for_compaction(max_chars);
      if content.is_empty() {
        return None;
      }
      Some(Message {
        id: message.id,
        msg_type: None,
        role: message.role.clone(),
        content,
        tool_call_id: None,
        tool_calls: None,
        timestamp: message.timestamp,
      })
    })
    .collect();

  let selected: Vec<&Message> = match last_summary {
    Some(ref summary) => {
      let mut selected = Vec::new();
      if let Some(first_user_message) =
        serialized.iter().find(|message| message.role == "user")
      {
        selected.push(first_user_message);
      }
      selected.extend(
        serialized
          .iter()
          .filter(|message| message.id > summary.covers_message_id),
      );
      selected
    }
    None => serialized.iter().collect(),
  };

  let transcript = selected
    .iter()
    .map(|message| message.content.as_str())
    .collect::<Vec<&str>>()
    .join("\n\n");
  let content =
    format!("Conversation transcript:\n{transcript}\n\n{SUMMARY_INSTRUCTION}");

  return PromptSpec {
    kind: PromptKind::Compaction,
    model: model.clone(),
    system: system_prompt,
    messages: vec![Message {
      id: Uuid::now_v7(),
      msg_type: None,
      role: "user".to_string(),
      content,
      tool_call_id: None,
      tool_calls: None,
      timestamp: chrono::Utc::now().timestamp(),
    }],
    tools: Vec::new(),
    stream: false,
    emit_tokens: false,
  };
}

pub async fn run_once(
  session: &mut Session,
  config: &Config,
  providers: &ProviderManager,
  repo: &RepoHandle,
  events: &Sender<SessionEvent>,
  correlation: Correlation,
  cancel: &CancellationToken,
) -> Result<String> {
  if session.messages.is_empty() {
    vlog!("Failed to compact session: no messages in session");
    bail!("No messages in session");
  }
  let Some(model) = session.model.clone() else {
    bail!("No model selected");
  };

  let spec = build(
    config,
    &session.cwd,
    &model,
    session.messages.clone(),
    session.summaries.last().cloned(),
  );
  let output = PromptExecutor::new(providers, config)
    .once(spec, correlation, events, cancel)
    .await?;

  session.last_usage = output.usage;

  let Some(content) = output.content else {
    bail!("Compaction workflow returned no content");
  };
  let summary = content.trim().to_string();
  if summary.is_empty() {
    bail!("Compaction workflow returned empty content");
  }

  let covers_message_id =
    session.messages.last().context("No messages to cover")?.id;
  session.add_summary(&summary, covers_message_id);
  let summary = session
    .summaries
    .last()
    .context("No summary to persist")?
    .clone();
  repo.insert_summary(session.id, summary.clone()).await?;

  vlog!("Compacted session: {}", summary.content);
  let _ = events
    .send(SessionEvent::Event(ServerEvent::Compacted {
      session_id: session.id,
      summary: summary.content.clone(),
    }))
    .await;
  return Ok(summary.content);
}

pub async fn run_if_needed(
  session: &mut Session,
  config: &Config,
  providers: &ProviderManager,
  repo: &RepoHandle,
  events: &Sender<SessionEvent>,
  correlation: Correlation,
  cancel: &CancellationToken,
) -> Result<()> {
  if !reached_compaction_threshold(session, config) {
    return Ok(());
  }
  run_once(
    session,
    config,
    providers,
    repo,
    events,
    correlation,
    cancel,
  )
  .await?;
  return Ok(());
}

fn reached_compaction_threshold(session: &Session, config: &Config) -> bool {
  let Some(threshold) = config.compaction.threshold else {
    return false;
  };
  let Some(last_usage) = session.last_usage else {
    return false;
  };
  return last_usage.prompt_tokens as usize
    >= (threshold as f64 * 0.8) as usize;
}
