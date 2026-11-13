pub mod chat;
pub mod compaction;
pub mod judge;
pub mod prediction;
pub mod review;
pub mod system;
pub mod title;

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Error, Result, bail};
use futures::StreamExt;
use tokio::sync::mpsc::Sender;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::engine::protocol::{Correlation, ServerEvent, SessionEvent};
use crate::network::retry::{self, RetryDelay, RetryPolicy};
use crate::persona_manager::Persona;
use crate::provider_manager::{ChatUsage, Model, ProviderManager, ToolCall};
use crate::session_manager::Message;
use crate::skill_manager::SkillRegistry;
use crate::tool_manager::tool::ToolSpec;
use crate::tool_manager::{
  EDIT_FILE_TOOL_NAME, GOAL_CHECK_TOOL_NAME, SUBAGENT_TOOL_NAME,
  TODO_LIST_TOOL_NAME, WRITE_FILE_TOOL_NAME,
};
use crate::utils::datetime::get_current_datetime;
use crate::vlog;

const TOOLS: &str = "Use tools when appropriate.";
const EDIT: &str = "Before editing files, read them with `append_loc=true` for accurate line numbers. Prefer many small edits over one large one.";
const GOAL: &str = "When the user gives a goal, do not try to finish it yourself. Delegate it with `subagent`, including the goal and its acceptance criteria. When the subagent returns, you must call `goal_check` with the goal before reporting anything. If the judge reports `Not achieved` or lists gaps, rewrite the goal to include them and delegate again with `subagent`. Repeat until the judge confirms the goal is achieved. Only then report to the user.";
const TODO_LIST: &str = "For complex multi-step work, maintain a todo list with `todo_list`: keep exactly one item in_progress at a time, mark items completed as you go, and update the list after each step.";
const VERIFY: &str = "After making changes, verify they work.";
const CWD_RESTRICTED: &str = "Tools are restricted to the current working directory. Always use paths within the current directory; paths outside the current working directory are blocked.";
const CONCISE: &str = "Be concise and direct.";
const PATHS: &str = "Show file paths clearly when working with files.";
const MENTIONS: &str = "Files mentioned with @path are automatically attached to the message (as text, or as an image). Never use read_file on an attached file.";
const SKILLS: &str = "Skills stay active for the whole session, whether you load one with the skill tool or the user attaches one with @skill-id. Apply a skill immediately. Never announce that it is loaded, describe it, or ask the user for input before acting. Never call the skill tool for a skill the user attached with @skill-id.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
  Chat,
  Title,
  Compaction,
  Prediction,
}

pub struct PromptSpec {
  pub kind: PromptKind,
  pub model: Model,
  pub system: String,
  pub messages: Vec<Message>,
  pub tools: Vec<ToolSpec>,
  pub stream: bool,
  pub emit_tokens: bool,
}

pub struct PromptOutput {
  pub content: Option<String>,
  pub reasoning: Option<String>,
  pub tool_calls: Vec<ToolCall>,
  pub usage: Option<ChatUsage>,
}

pub struct PromptExecutor<'a> {
  providers: &'a ProviderManager,
  config: &'a Config,
}

impl<'a> PromptExecutor<'a> {
  pub fn new(providers: &'a ProviderManager, config: &'a Config) -> Self {
    return Self { providers, config };
  }

  pub async fn once(
    &self,
    spec: PromptSpec,
    skills: Option<&SkillRegistry>,
    correlation: Correlation,
    events: &Sender<SessionEvent>,
    cancel: &CancellationToken,
  ) -> Result<PromptOutput> {
    vlog!("Running prompt: {:?}", spec.kind);
    if spec.stream && self.config.general.stream {
      return self
        .once_streaming(spec, skills, correlation, events, cancel)
        .await;
    }
    return self
      .once_blocking(spec, skills, correlation, events, cancel)
      .await;
  }

  async fn with_retries<T, F, Fut>(
    &self,
    kind: PromptKind,
    correlation: Correlation,
    events: &Sender<SessionEvent>,
    cancel: &CancellationToken,
    mut attempt: F,
  ) -> Result<T>
  where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
  {
    let policy = RetryPolicy::from_config(&self.config.network);
    let max_attempts = policy.max_attempts();
    let mut retries = 0u32;

    loop {
      let result = tokio::select! {
          biased;
          _ = cancel.cancelled() => bail!("Cancelled"),
          result = attempt() => result,
      };

      let error = match result {
        Ok(value) => return Ok(value),
        Err(error) => error,
      };

      if retries >= policy.max_retries {
        return Err(final_error(error, retries));
      }
      let Some(advice) = retry::classify(&error) else {
        return Err(error);
      };
      let delay = match policy.delay(retries + 1, advice) {
        RetryDelay::Wait(delay) => delay,
        RetryDelay::TooLong(requested) => {
          return Err(error.context(format!(
            "Provider asked to retry after {}s, above the configured limit of {}s",
            requested.as_secs(),
            policy.max_delay.as_secs()
          )));
        }
      };

      retries += 1;
      let next_attempt = retries + 1;
      let reason = retry::reason(&error);
      vlog!(
        "Retrying request in {}s (attempt {}/{}): {}",
        delay.as_secs(),
        next_attempt,
        max_attempts,
        reason
      );
      if matches!(kind, PromptKind::Chat | PromptKind::Compaction) {
        let _ = events
          .send(SessionEvent::Event(ServerEvent::Retrying {
            correlation,
            attempt: next_attempt,
            max_attempts,
            delay_seconds: delay.as_secs(),
            reason,
          }))
          .await;
      }

      tokio::select! {
        biased;
        _ = cancel.cancelled() => bail!("Cancelled"),
        _ = sleep(delay) => {}
      }
    }
  }

  async fn once_blocking(
    &self,
    spec: PromptSpec,
    skills: Option<&SkillRegistry>,
    correlation: Correlation,
    events: &Sender<SessionEvent>,
    cancel: &CancellationToken,
  ) -> Result<PromptOutput> {
    let messages = with_system(&spec.system, &spec.messages);
    let response = self
      .with_retries(spec.kind, correlation, events, cancel, || {
        self
          .providers
          .chat(&spec.model, &messages, &spec.tools, skills)
      })
      .await?;
    if spec.emit_tokens {
      if self.config.general.show_reasoning
        && let Some(reasoning) = response.reasoning.as_deref()
      {
        let _ = events
          .send(SessionEvent::Event(ServerEvent::Token {
            correlation,
            text: reasoning.to_string(),
            reasoning: true,
          }))
          .await;
      }
      if let Some(content) = response.content.as_deref() {
        let _ = events
          .send(SessionEvent::Event(ServerEvent::Token {
            correlation,
            text: content.to_string(),
            reasoning: false,
          }))
          .await;
      }
    }
    return Ok(PromptOutput {
      content: response.content,
      reasoning: response.reasoning,
      tool_calls: response.tool_calls,
      usage: response.usage,
    });
  }

  async fn once_streaming(
    &self,
    spec: PromptSpec,
    skills: Option<&SkillRegistry>,
    correlation: Correlation,
    events: &Sender<SessionEvent>,
    cancel: &CancellationToken,
  ) -> Result<PromptOutput> {
    let messages = with_system(&spec.system, &spec.messages);
    let mut stream = self
      .with_retries(spec.kind, correlation, events, cancel, || {
        self
          .providers
          .chat_stream(&spec.model, &messages, &spec.tools, skills)
      })
      .await?;

    let mut content = String::new();
    let mut reasoning = String::new();
    let mut usage = None;
    let mut tool_calls: HashMap<usize, ToolCall> = HashMap::new();
    let idle_seconds = self.config.network.stream_idle_timeout_seconds;

    loop {
      let delta = tokio::select! {
          biased;
          _ = cancel.cancelled() => bail!("Cancelled"),
          chunk = stream.next() => match chunk {
              Some(Ok(chunk)) => chunk,
              Some(Err(error)) => {
                  return Err(error.context(format!(
                      "Connection lost after {} characters of output; the partial response was discarded",
                      content.len() + reasoning.len()
                  )));
              }
              None => break,
          },
          _ = idle_timeout(idle_seconds) => {
              bail!(
                  "Provider stopped sending data for {idle_seconds}s after {} characters of output; the partial response was discarded",
                  content.len() + reasoning.len()
              );
          }
      };

      if let Some(value) = delta.usage {
        usage = Some(value);
      }
      if let Some(text) = delta.content {
        content.push_str(&text);
        if spec.emit_tokens {
          let _ = events
            .send(SessionEvent::Event(ServerEvent::Token {
              correlation,
              text,
              reasoning: false,
            }))
            .await;
        }
      }
      if let Some(text) = delta.reasoning {
        reasoning.push_str(&text);
        if spec.emit_tokens && self.config.general.show_reasoning {
          let _ = events
            .send(SessionEvent::Event(ServerEvent::Token {
              correlation,
              text,
              reasoning: true,
            }))
            .await;
        }
      }
      for call in delta.tool_calls {
        let entry = tool_calls.entry(call.index).or_insert_with(|| ToolCall {
          id: String::new(),
          name: String::new(),
          arguments: String::new(),
        });
        if let Some(id) = call.id {
          entry.id = id;
        }
        if let Some(name) = call.name {
          entry.name = name;
        }
        if let Some(args) = call.arguments {
          entry.arguments.push_str(&args);
        }
      }
    }

    let mut tool_calls: Vec<(usize, ToolCall)> =
      tool_calls.into_iter().collect();
    tool_calls.sort_by_key(|(index, _)| *index);
    return Ok(PromptOutput {
      content: (!content.is_empty()).then_some(content),
      reasoning: (!reasoning.is_empty()).then_some(reasoning),
      tool_calls: tool_calls.into_iter().map(|(_, call)| call).collect(),
      usage,
    });
  }
}

fn final_error(error: Error, retries: u32) -> Error {
  if retries == 0 {
    return error;
  }
  return error.context(format!(
    "Request failed after {} attempts",
    retries.saturating_add(1)
  ));
}

async fn idle_timeout(seconds: u64) {
  if seconds == 0 {
    return std::future::pending::<()>().await;
  }
  return sleep(Duration::from_secs(seconds)).await;
}

pub fn with_system(system: &str, messages: &[Message]) -> Vec<Message> {
  let mut messages = messages.to_vec();
  if let Some(first) = messages.first_mut()
    && first.role == "system"
  {
    first.content = system.to_string();
    return messages;
  }
  messages.insert(
    0,
    Message {
      id: Uuid::now_v7(),
      msg_type: None,
      role: "system".to_string(),
      content: system.to_string(),
      tool_call_id: None,
      tool_calls: None,
      timestamp: chrono::Utc::now().timestamp(),
    },
  );
  return messages;
}

pub struct Prompt {
  system_prompt: String,
}

impl Prompt {
  pub fn new(config: &Config, persona: &Persona, cwd: &str) -> Self {
    let mut parts: Vec<String> = Vec::new();
    parts.push(persona.identity().to_string());

    let mut guidelines: Vec<&str> = Vec::new();
    for instruction in persona.instructions() {
      guidelines.push(instruction);
    }
    let can_edit = persona.allowed_tools().allows(EDIT_FILE_TOOL_NAME);
    let can_write =
      can_edit || persona.allowed_tools().allows(WRITE_FILE_TOOL_NAME);

    guidelines.push(TOOLS);
    if config.permissions.read_file && config.permissions.edit_file && can_edit
    {
      guidelines.push(EDIT);
    }
    if config.general.restrict_write_tools_to_cwd && can_write {
      guidelines.push(CWD_RESTRICTED);
    }
    if can_write {
      guidelines.push(VERIFY);
    }
    if config.subagent.enabled
      && config.permissions.goal_check
      && persona.allowed_tools().allows(SUBAGENT_TOOL_NAME)
      && persona.allowed_tools().allows(GOAL_CHECK_TOOL_NAME)
    {
      guidelines.push(GOAL);
    }
    if config.permissions.todo_list
      && persona.allowed_tools().allows(TODO_LIST_TOOL_NAME)
    {
      guidelines.push(TODO_LIST);
    }
    guidelines.push(CONCISE);
    guidelines.push(PATHS);
    guidelines.push(MENTIONS);
    guidelines.push(SKILLS);

    let guidelines_list = guidelines
      .iter()
      .map(|g| format!("- {g}"))
      .collect::<Vec<String>>()
      .join("\n");
    parts.push(format!("Guidelines:\n{guidelines_list}"));

    let cwd = if cwd.trim().is_empty() {
      String::new()
    } else {
      format!("Current working directory: {cwd}\n")
    };
    let date_time = get_current_datetime();
    parts.push(format!("Environment:\n{cwd}{date_time}"));

    let system_prompt = parts.join("\n\n");
    return Self { system_prompt };
  }

  pub fn get_system_prompt(&self) -> &str {
    return &self.system_prompt;
  }
}
