use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::app::AppEvent;
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::config::Config;
use crate::hook_manager::HookManager;
use crate::hook_manager::hook::{
  HookContext, HookDecision, ToolCallContext, ToolResultContext,
};
use crate::mentions::strip_mentions;
use crate::persona_manager::PersonaManager;
use crate::prompt::Prompt;
use crate::provider_manager::{ChatUsage, ProviderManager, ToolCall};
use crate::session_manager::SessionManager;
use crate::tool_manager::SUBAGENT_TOOL_NAME;
use crate::tool_manager::ToolManager;
use crate::tool_manager::tool::{ToolContext, parse_tool_params};
use crate::vlog;
use crate::workflow_manager::WorkflowManager;
use crate::workflow_manager::title::TitleParams;

#[derive(Debug, Clone, Copy)]
pub enum AgentKind {
  Primary,
  Subagent,
}

pub struct Agent {
  pub config: Config,
  pub tool_manager: ToolManager,
  pub persona_manager: PersonaManager,
  pub provider_manager: Arc<ProviderManager>,
  pub workflow_manager: WorkflowManager,
  pub hook_manager: HookManager,
  pub last_usage: Option<ChatUsage>,
  pub kind: AgentKind,
}

impl Agent {
  pub fn new(
    config: Config,
    provider_manager: Arc<ProviderManager>,
    parent_session_id: Option<Uuid>,
  ) -> Self {
    let kind = match parent_session_id {
      Some(_) => AgentKind::Subagent,
      None => AgentKind::Primary,
    };
    let tool_manager = ToolManager::new(&config, &kind);
    let persona_manager = PersonaManager::new();
    let workflow_manager =
      WorkflowManager::new(config.clone(), provider_manager.clone());
    let hook_manager = HookManager::new(&config);
    return Agent {
      config,
      tool_manager,
      persona_manager,
      provider_manager,
      workflow_manager,
      hook_manager,
      last_usage: None,
      kind,
    };
  }

  pub async fn run_subagent(
    context: &mut ToolContext<'_>,
    task: &str,
  ) -> RuntimeResult<String> {
    let task = task.trim();
    if task.is_empty() {
      return Err(RuntimeError::Subagent("Task cannot be empty".to_string()));
    }
    context.tx.send(AppEvent::Subagents { delta: 1 }).await.ok();
    let result = Self::run_subagent_inner(context, task).await;
    context
      .tx
      .send(AppEvent::Subagents { delta: -1 })
      .await
      .ok();
    return result;
  }

  async fn run_subagent_inner(
    context: &mut ToolContext<'_>,
    task: &str,
  ) -> RuntimeResult<String> {
    let parent_session_id = context.session_id;
    let session_id = context
      .session_manager
      .create_session(Some(parent_session_id))
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;
    vlog!(
      "Started subagent session {session_id} (parent {parent_session_id}): {task}"
    );

    let prompt = Prompt::new(context.config, context.persona);
    context
      .session_manager
      .add_system_message_to_session(session_id, prompt.get_system_prompt())
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;
    context
      .session_manager
      .add_user_message_to_session(session_id, task)
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;

    let mut agent = Self::new(
      context.config.clone(),
      context.provider_manager.clone(),
      Some(parent_session_id),
    );
    agent.persona_manager.set_persona(context.persona.clone());

    let result = agent
      .process_chat(context.session_manager, context.tx, context.cancel)
      .await;

    context
      .session_manager
      .set_current_session(parent_session_id);

    let report = result?.trim().to_string();
    if report.is_empty() {
      if context.cancel.is_cancelled() {
        vlog!("Subagent session {session_id} was cancelled.");
        return Ok(
          "The subagent was cancelled before producing a report.".to_string(),
        );
      }
      vlog!("Subagent session {session_id} finished without a report.");
      return Ok(
        "The subagent finished without producing a report.".to_string(),
      );
    }
    vlog!("Subagent session {session_id} finished.");
    return Ok(report);
  }

  pub async fn process_chat(
    &mut self,
    session_manager: &mut SessionManager,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> RuntimeResult<String> {
    if cancel.is_cancelled() {
      return Ok(String::new());
    }
    let is_subagent = matches!(self.kind, AgentKind::Subagent);
    if !is_subagent {
      self
        .generate_title_from_workflow(session_manager, cancel)
        .await?;
    }
    if self.config.general.stream {
      return self
        .process_chat_stream(session_manager, tx, cancel, is_subagent)
        .await;
    }
    return self
      .process_chat_non_streaming(session_manager, tx, cancel, is_subagent)
      .await;
  }

  async fn generate_title_from_workflow(
    &mut self,
    session_manager: &mut SessionManager,
    cancel: &CancellationToken,
  ) -> RuntimeResult<()> {
    let session = match session_manager.get_current_session() {
      Some(session) if session.title.is_none() => session.clone(),
      _ => return Ok(()),
    };

    let first_message = session
      .messages
      .iter()
      .find(|m| m.role == "user")
      .map(|m| m.content.clone())
      .map(|content| strip_mentions(&content))
      .unwrap_or_default();
    if first_message.is_empty() {
      return Ok(());
    }

    let params = TitleParams {
      message: first_message,
    };

    let title = match tokio::select! {
      biased;
      _ = cancel.cancelled() => {
        vlog!("Session title generation cancelled.");
        return Ok(());
      }
      result = self.workflow_manager.generate_title(params) => result,
    } {
      Ok(title) => title,
      Err(e) => {
        vlog!("Failed to generate session title: {e}");
        return Ok(());
      }
    };

    if title.is_empty() {
      return Ok(());
    }

    session_manager
      .update_session_title(session.id, &title)
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;
    return Ok(());
  }

  async fn process_chat_stream(
    &mut self,
    session_manager: &mut SessionManager,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
    is_subagent: bool,
  ) -> RuntimeResult<String> {
    let mut assistant_content = String::new();

    loop {
      let session = session_manager
        .get_current_session()
        .ok_or(RuntimeError::Session("No active session".into()))?;
      let session_id = session.id;

      let messages = &session.build_chat_context();
      let tools = self
        .tool_manager
        .get_tools_specs(self.persona_manager.current().allowed_tools());

      let mut stream = self
        .provider_manager
        .chat_stream(messages, &tools)
        .await
        .map_err(|e| RuntimeError::Provider(e.to_string()))?;

      let mut content_buffer = String::new();
      let mut reasoning_buffer = String::new();
      let mut stream_usage: Option<ChatUsage> = None;
      let mut tool_call_builders: HashMap<usize, ToolCall> = HashMap::new();

      loop {
        let delta = tokio::select! {
          biased;
          _ = cancel.cancelled() => {
            vlog!("Stream cancelled by user.");
            break;
          }
          chunk_result = stream.next() => match chunk_result {
            Some(result) => result.map_err(|e| RuntimeError::Provider(e.to_string()))?,
            None => break,
          },
        };

        if let Some(usage) = delta.usage {
          stream_usage = Some(usage);
        }

        if let Some(text) = delta.content {
          content_buffer.push_str(&text);
          if !is_subagent {
            tx.send(AppEvent::Token {
              text,
              reasoning: false,
            })
            .await
            .ok();
          }
        }

        if let Some(text) = delta.reasoning {
          reasoning_buffer.push_str(&text);
          if self.config.general.show_reasoning && !is_subagent {
            tx.send(AppEvent::Token {
              text,
              reasoning: true,
            })
            .await
            .ok();
          }
        }

        for tc_delta in delta.tool_calls {
          let entry =
            tool_call_builders
              .entry(tc_delta.index)
              .or_insert(ToolCall {
                id: String::new(),
                name: String::new(),
                arguments: String::new(),
              });
          if let Some(id) = tc_delta.id {
            entry.id = id;
          }
          if let Some(name) = tc_delta.name {
            entry.name = name;
          }
          if let Some(args) = tc_delta.arguments {
            entry.arguments.push_str(&args);
          }
        }
      }

      if let Some(usage) = stream_usage {
        self.last_usage = Some(usage);
      }

      if !content_buffer.is_empty() {
        assistant_content.push_str(&content_buffer);
      }

      if self.config.general.show_reasoning && !reasoning_buffer.is_empty() {
        session_manager
          .add_reasoning_message_to_session(session_id, &reasoning_buffer)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }
      if !content_buffer.is_empty() || tool_call_builders.is_empty() {
        session_manager
          .add_assistant_message_to_session(session_id, &content_buffer)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }

      if tool_call_builders.is_empty() {
        break;
      }

      let tool_calls: Vec<ToolCall> =
        tool_call_builders.into_values().collect();
      self
        .execute_tool_calls(
          session_manager,
          session_id,
          &tool_calls,
          tx,
          cancel,
          is_subagent,
        )
        .await?;
    }

    return Ok(assistant_content);
  }

  async fn process_chat_non_streaming(
    &mut self,
    session_manager: &mut SessionManager,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
    is_subagent: bool,
  ) -> RuntimeResult<String> {
    let mut assistant_content = String::new();

    loop {
      if cancel.is_cancelled() {
        break;
      }
      let session = session_manager
        .get_current_session()
        .ok_or(RuntimeError::Session("No active session".into()))?;
      let session_id = session.id;

      let messages = &session.build_chat_context();
      let tools = self
        .tool_manager
        .get_tools_specs(self.persona_manager.current().allowed_tools());

      let response = self
        .provider_manager
        .chat(messages, &tools)
        .await
        .map_err(|e| RuntimeError::Provider(e.to_string()))?;

      if let Some(usage) = response.usage {
        self.last_usage = Some(usage);
      }

      if self.config.general.show_reasoning
        && !is_subagent
        && let Some(ref reasoning) = response.reasoning
      {
        tx.send(AppEvent::Token {
          text: reasoning.clone(),
          reasoning: true,
        })
        .await
        .ok();
      }

      if let Some(ref content) = response.content {
        if !is_subagent {
          tx.send(AppEvent::Token {
            text: content.clone(),
            reasoning: false,
          })
          .await
          .ok();
        }
        assistant_content.push_str(content);
      }

      if self.config.general.show_reasoning
        && let Some(ref reasoning) = response.reasoning
        && !reasoning.is_empty()
      {
        session_manager
          .add_reasoning_message_to_session(session_id, reasoning)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }
      if let Some(ref content) = response.content {
        session_manager
          .add_assistant_message_to_session(session_id, content)
          .await
          .map_err(|e| RuntimeError::Session(e.to_string()))?;
      }

      if response.tool_calls.is_empty() {
        break;
      }

      self
        .execute_tool_calls(
          session_manager,
          session_id,
          &response.tool_calls,
          tx,
          cancel,
          is_subagent,
        )
        .await?;
    }

    return Ok(assistant_content);
  }

  async fn execute_tool_calls(
    &mut self,
    session_manager: &mut SessionManager,
    session_id: Uuid,
    tool_calls: &[ToolCall],
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
    is_subagent: bool,
  ) -> RuntimeResult<()> {
    for tool_call in tool_calls {
      if cancel.is_cancelled() {
        break;
      }
      vlog!(
        "Tool call: {} with arguments: {}",
        tool_call.name,
        tool_call.arguments
      );

      session_manager
        .add_tool_call_to_session(
          session_id,
          &tool_call.id,
          &tool_call.name,
          &tool_call.arguments,
        )
        .await
        .map_err(|e| RuntimeError::Session(e.to_string()))?;

      let params = parse_tool_params(&tool_call.arguments);
      let tool_is_subagent = tool_call.name == SUBAGENT_TOOL_NAME;

      if self.config.general.show_tools && !is_subagent && !tool_is_subagent {
        tx.send(AppEvent::ToolCall {
          name: tool_call.name.clone(),
          arguments: tool_call.arguments.clone(),
        })
        .await
        .ok();
      }

      let hook_context = HookContext {
        session_id,
        config: &self.config,
        persona: self.persona_manager.current(),
        agent_kind: self.kind,
        tx,
      };
      let tool_call_context = ToolCallContext {
        name: &tool_call.name,
        params: &params,
        is_read_only: self.tool_manager.is_read_only(&tool_call.name),
      };
      let decision = self
        .hook_manager
        .pre_tool_use(&hook_context, &tool_call_context)
        .await;

      match decision {
        HookDecision::Continue | HookDecision::Allow => {}
        HookDecision::Deny { reason, error } => {
          session_manager
            .add_tool_result_to_session(session_id, &tool_call.id, &reason)
            .await
            .map_err(|e| RuntimeError::Session(e.to_string()))?;
          if self.config.general.show_tools && !is_subagent {
            tx.send(AppEvent::ToolResult {
              result: reason,
              error,
            })
            .await
            .ok();
          }
          continue;
        }
      }

      if self.config.general.show_tools && !is_subagent && tool_is_subagent {
        let task = params.get("task").cloned().unwrap_or_default();
        tx.send(AppEvent::SubagentStarted { task }).await.ok();
      }

      let mut context = ToolContext {
        config: &self.config,
        persona: self.persona_manager.current(),
        provider_manager: &self.provider_manager,
        session_manager: &mut *session_manager,
        tx,
        cancel,
        session_id,
      };
      let result = self
        .tool_manager
        .execute_tool(&tool_call.name, params, &mut context)
        .await;

      let (mut result_str, error) = match result {
        Ok(r) => {
          vlog!("Tool '{}' result: {}", tool_call.name, r);
          (r, false)
        }
        Err(e) => {
          vlog!("Tool '{}' error: {}", tool_call.name, e);
          (format!("Tool error: {e}"), true)
        }
      };

      let tool_result_context = ToolResultContext {
        name: &tool_call.name,
        arguments: &tool_call.arguments,
        error: &error,
      };
      let hook_failure = self
        .hook_manager
        .post_tool_use(&hook_context, &tool_result_context)
        .await;

      if !hook_failure.is_empty() {
        vlog!(
          "Post tool use hook failure for '{}': {}",
          tool_call.name,
          hook_failure
        );
        result_str.push_str(&hook_failure);
      }

      session_manager
        .add_tool_result_to_session(session_id, &tool_call.id, &result_str)
        .await
        .map_err(|e| RuntimeError::Session(e.to_string()))?;

      if self.config.general.show_tools
        && !is_subagent
        && (!tool_is_subagent || error)
      {
        tx.send(AppEvent::ToolResult {
          result: result_str,
          error,
        })
        .await
        .ok();
      }
    }
    return Ok(());
  }
}
