use anyhow::{Context, Result, bail};
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::engine::protocol::{Correlation, ServerEvent, SessionEvent};
use crate::engine::repo_actor::RepoHandle;
use crate::persona_manager::COMPACTION_PERSONA;
use crate::prompt::{PromptExecutor, PromptKind, PromptSpec, system};
use crate::provider_manager::{Model, ProviderManager};
use crate::session_manager::session::Session;
use crate::session_manager::{Message, Summary};
use crate::vlog;

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
        msg_type: message.msg_type.clone(),
        role: message.role.clone(),
        content,
        tool_call_id: message.tool_call_id.clone(),
        tool_calls: message.tool_calls.clone(),
        timestamp: message.timestamp,
      })
    })
    .collect();

  let mut prompt_messages = Vec::new();
  if let Some(ref summary) = last_summary {
    let first_user_message =
      serialized.iter().find(|message| message.role == "user");
    if let Some(message) = first_user_message {
      prompt_messages.push(message.clone());
    }
    for message in &serialized {
      if message.id > summary.covers_message_id {
        prompt_messages.push(message.clone());
      }
    }
  } else {
    prompt_messages = serialized;
  }

  return PromptSpec {
    kind: PromptKind::Compaction,
    model: model.clone(),
    system: system_prompt,
    messages: prompt_messages,
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
