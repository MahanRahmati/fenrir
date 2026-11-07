pub mod chat;
pub mod compaction;
pub mod judge;
pub mod prediction;
pub mod review;
pub mod system;
pub mod title;

use std::collections::HashMap;

use anyhow::{Result, bail};
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::engine::protocol::{Correlation, ServerEvent, SessionEvent};
use crate::persona_manager::Persona;
use crate::provider_manager::{ChatUsage, Model, ProviderManager, ToolCall};
use crate::session_manager::Message;
use crate::tool_manager::tool::ToolSpec;
use crate::tool_manager::{
  EDIT_FILE_TOOL_NAME, GOAL_CHECK_TOOL_NAME, SUBAGENT_TOOL_NAME,
  TODO_LIST_TOOL_NAME, WRITE_FILE_TOOL_NAME,
};
use crate::utils::datetime::get_current_datetime;

const TOOLS: &str = "Use tools when appropriate.";
const EDIT: &str = "Before editing files, read them with `append_loc=true` for accurate line numbers. Prefer many small edits over one large one.";
const GOAL: &str = "When the user gives a goal, do not try to finish it yourself. Delegate it with `subagent`, including the goal and its acceptance criteria. When the subagent returns, you must call `goal_check` with the goal before reporting anything. If the judge reports `Not achieved` or lists gaps, rewrite the goal to include them and delegate again with `subagent`. Repeat until the judge confirms the goal is achieved. Only then report to the user.";
const TODO_LIST: &str = "For complex multi-step work, maintain a todo list with `todo_list`: keep exactly one item in_progress at a time, mark items completed as you go, and update the list after each step.";
const VERIFY: &str = "After making changes, verify they work.";
const CWD_RESTRICTED: &str = "Tools are restricted to the current working directory. Always use paths within the current directory; paths outside the current working directory are blocked.";
const CONCISE: &str = "Be concise and direct.";
const PATHS: &str = "Show file paths clearly when working with files.";
const MENTIONS: &str = "Files mentioned with @path are automatically attached to the message (as text, or as an image). Never use read_file on an attached file.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
  Chat,
  Title,
  Compaction,
  Prediction,
  Judge,
  Subagent,
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
    correlation: Correlation,
    events: &mpsc::Sender<SessionEvent>,
    cancel: &CancellationToken,
  ) -> Result<PromptOutput> {
    if spec.stream && self.config.general.stream {
      return self.once_streaming(spec, correlation, events, cancel).await;
    }
    return self.once_blocking(spec, cancel).await;
  }

  async fn once_blocking(
    &self,
    spec: PromptSpec,
    cancel: &CancellationToken,
  ) -> Result<PromptOutput> {
    let messages = with_system(&spec.system, &spec.messages);
    let response = tokio::select! {
        biased;
        _ = cancel.cancelled() => bail!("Cancelled"),
        response = self.providers.chat(
            &spec.model,
            &messages,
            &spec.tools,
        ) => response?,
    };
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
    correlation: Correlation,
    events: &mpsc::Sender<SessionEvent>,
    cancel: &CancellationToken,
  ) -> Result<PromptOutput> {
    let messages = with_system(&spec.system, &spec.messages);
    let mut stream = tokio::select! {
        biased;
        _ = cancel.cancelled() => bail!("Cancelled"),
        stream = self.providers.chat_stream(
            &spec.model,
            &messages,
            &spec.tools,
        ) => stream?,
    };

    let mut content = String::new();
    let mut reasoning = String::new();
    let mut usage = None;
    let mut tool_calls: HashMap<usize, ToolCall> = HashMap::new();

    loop {
      let delta = tokio::select! {
          biased;
          _ = cancel.cancelled() => bail!("Cancelled"),
          chunk = stream.next() => match chunk {
              Some(chunk) => chunk?,
              None => break,
          },
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
