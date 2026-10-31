use std::collections::HashMap;

use anyhow::{Result, bail};
use futures::StreamExt;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

use crate::app::AppEvent;
use crate::config::Config;
use crate::hook_manager::hook::{
  HookContext, HookDecision, ToolCallContext, ToolResultContext,
};
use crate::mentions::strip_mentions;
use crate::persona_manager::Persona;
use crate::provider_manager::{ChatUsage, Model, ProviderManager, ToolCall};
use crate::session_manager::SessionRepository;
use crate::session_manager::session::Session;
use crate::tool_manager::SUBAGENT_TOOL_NAME;
use crate::tool_manager::runs_subagent;
use crate::tool_manager::tool::{ToolContext, parse_tool_params};
use crate::vlog;
use crate::workflow_manager::title::TitleParams;

impl Session {
  pub async fn harness_loop(
    &mut self,
    config: &Config,
    providers: &ProviderManager,
    repository: &SessionRepository,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> Result<String> {
    if cancel.is_cancelled() {
      return Ok(String::new());
    }
    let Some(model) = self.model.clone() else {
      bail!("No model selected");
    };
    if !self.is_subagent() {
      self
        .generate_title(providers, repository, &model, cancel)
        .await?;
    }
    if config.general.stream {
      return self
        .process_chat_stream(config, providers, repository, &model, tx, cancel)
        .await;
    }
    return self
      .process_chat_non_streaming(
        config, providers, repository, &model, tx, cancel,
      )
      .await;
  }

  async fn generate_title(
    &mut self,
    providers: &ProviderManager,
    repository: &SessionRepository,
    model: &Model,
    cancel: &CancellationToken,
  ) -> Result<()> {
    if self.title.is_some() {
      return Ok(());
    }

    let first_message = self
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
      result = self
        .workflow_manager
        .generate_title(providers, model, params) => result,
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

    self.title = Some(title.clone());
    repository.update_title(self.id, &title).await?;
    return Ok(());
  }

  async fn process_chat_stream(
    &mut self,
    config: &Config,
    providers: &ProviderManager,
    repository: &SessionRepository,
    model: &Model,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> Result<String> {
    let is_subagent = self.is_subagent();
    let mut assistant_content = String::new();
    let max_iterations = config.subagent.max_iterations;
    let mut turns = 0;

    loop {
      if cancel.is_cancelled() {
        break;
      }
      if is_subagent && turns >= max_iterations {
        assistant_content.push_str(&subagent_turn_limit_note(max_iterations));
        break;
      }
      turns += 1;

      let messages = self.build_chat_context();
      let tools = self
        .tool_manager
        .get_tools_specs(self.persona_manager.current().allowed_tools());

      let mut stream = tokio::select! {
        biased;
        _ = cancel.cancelled() => break,
        stream = providers.chat_stream(model, &messages, &tools) => {
          stream?
        }
      };

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
            Some(result) => result?,
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
          if config.general.show_reasoning && !is_subagent {
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

      if config.general.show_reasoning && !reasoning_buffer.is_empty() {
        self.add_reasoning_message(&reasoning_buffer);
        self.persist_last_message(repository).await?;
      }
      if !content_buffer.is_empty() || tool_call_builders.is_empty() {
        self.add_assistant_message(&content_buffer);
        self.persist_last_message(repository).await?;
      }

      if tool_call_builders.is_empty() {
        break;
      }

      let tool_calls: Vec<ToolCall> =
        tool_call_builders.into_values().collect();
      self
        .execute_tool_calls(
          config,
          providers,
          repository,
          &tool_calls,
          tx,
          cancel,
        )
        .await?;
    }

    return Ok(assistant_content);
  }

  async fn process_chat_non_streaming(
    &mut self,
    config: &Config,
    providers: &ProviderManager,
    repository: &SessionRepository,
    model: &Model,
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> Result<String> {
    let is_subagent = self.is_subagent();
    let mut assistant_content = String::new();
    let max_iterations = config.subagent.max_iterations;
    let mut turns = 0;

    loop {
      if cancel.is_cancelled() {
        break;
      }
      if is_subagent && turns >= max_iterations {
        assistant_content.push_str(&subagent_turn_limit_note(max_iterations));
        break;
      }
      turns += 1;

      let messages = self.build_chat_context();
      let tools = self
        .tool_manager
        .get_tools_specs(self.persona_manager.current().allowed_tools());

      let response = tokio::select! {
        biased;
        _ = cancel.cancelled() => break,
        response = providers.chat(model, &messages, &tools) => {
          response?
        }
      };

      if let Some(usage) = response.usage {
        self.last_usage = Some(usage);
      }

      if config.general.show_reasoning
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

      if config.general.show_reasoning
        && let Some(ref reasoning) = response.reasoning
        && !reasoning.is_empty()
      {
        self.add_reasoning_message(reasoning);
        self.persist_last_message(repository).await?;
      }
      if let Some(ref content) = response.content {
        self.add_assistant_message(content);
        self.persist_last_message(repository).await?;
      }

      if response.tool_calls.is_empty() {
        break;
      }

      self
        .execute_tool_calls(
          config,
          providers,
          repository,
          &response.tool_calls,
          tx,
          cancel,
        )
        .await?;
    }

    return Ok(assistant_content);
  }

  async fn execute_tool_calls(
    &mut self,
    config: &Config,
    providers: &ProviderManager,
    repository: &SessionRepository,
    tool_calls: &[ToolCall],
    tx: &Sender<AppEvent>,
    cancel: &CancellationToken,
  ) -> Result<()> {
    let Some(model) = self.model.clone() else {
      bail!("No model selected");
    };
    let is_subagent = self.is_subagent();
    for tool_call in tool_calls {
      if cancel.is_cancelled() {
        break;
      }
      vlog!(
        "Tool call: {} with arguments: {}",
        tool_call.name,
        tool_call.arguments
      );

      self.add_tool_call(&tool_call.id, &tool_call.name, &tool_call.arguments);
      self.persist_last_message(repository).await?;

      if cancel.is_cancelled() {
        self.add_cancelled_tool_result(&tool_call.id);
        self.persist_last_message(repository).await?;
        break;
      }

      let params = parse_tool_params(&tool_call.arguments);
      let runs_subagent = runs_subagent(&tool_call.name);
      let is_subagent_tool = tool_call.name == SUBAGENT_TOOL_NAME;

      if config.general.show_tools && !is_subagent && !is_subagent_tool {
        tx.send(AppEvent::ToolCall {
          id: tool_call.id.clone(),
          name: tool_call.name.clone(),
          arguments: tool_call.arguments.clone(),
        })
        .await
        .ok();
      }

      let hook_context = HookContext {
        session_id: self.id,
        config,
        persona: self.persona_manager.current(),
        is_subagent,
        tx,
        repository,
        approvals: self.approval_channel(),
        cancel,
      };
      let tool_call_context = ToolCallContext {
        id: &tool_call.id,
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
          self.add_tool_result_message(&tool_call.id, &reason);
          self.persist_last_message(repository).await?;
          if config.general.show_tools && !is_subagent {
            tx.send(AppEvent::ToolResult {
              id: tool_call.id.clone(),
              result: reason,
              error,
            })
            .await
            .ok();
          }
          continue;
        }
      }

      if config.general.show_tools && !is_subagent && is_subagent_tool {
        let task = params.get("task").cloned().unwrap_or_default();
        tx.send(AppEvent::SubagentStarted { task }).await.ok();
      }

      let outcome = {
        let mut context = ToolContext {
          config,
          persona: self.persona_manager.current(),
          model: &model,
          provider_manager: providers,
          repository,
          approvals: self.approval_channel(),
          tx,
          cancel,
          session_id: self.id,
        };
        if runs_subagent {
          Some(
            self
              .tool_manager
              .execute_tool(&tool_call.name, params, &mut context)
              .await,
          )
        } else {
          tokio::select! {
            biased;
            _ = cancel.cancelled() => None,
            result = self
              .tool_manager
              .execute_tool(&tool_call.name, params, &mut context) => Some(result),
          }
        }
      };

      let Some(result) = outcome else {
        self.add_cancelled_tool_result(&tool_call.id);
        self.persist_last_message(repository).await?;
        break;
      };

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
      let hook_context = HookContext {
        session_id: self.id,
        config,
        persona: self.persona_manager.current(),
        is_subagent,
        tx,
        repository,
        approvals: self.approval_channel(),
        cancel,
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

      self.add_tool_result_message(&tool_call.id, &result_str);
      self.persist_last_message(repository).await?;

      if config.general.show_tools
        && !is_subagent
        && (!is_subagent_tool || error)
      {
        tx.send(AppEvent::ToolResult {
          id: tool_call.id.clone(),
          result: result_str,
          error,
        })
        .await
        .ok();
      }
    }
    return Ok(());
  }

  fn add_cancelled_tool_result(&mut self, tool_call_id: &str) {
    self.add_tool_result_message(tool_call_id, CANCELLED_TOOL_RESULT);
  }

  pub async fn run_subagent(
    context: &ToolContext<'_>,
    persona: &Persona,
    task: &str,
  ) -> Result<String> {
    let task = task.trim();
    if task.is_empty() {
      bail!("Task cannot be empty");
    }
    context.tx.send(AppEvent::Subagents { delta: 1 }).await.ok();
    let result = Self::run_subagent_inner(context, persona, task).await;
    context
      .tx
      .send(AppEvent::Subagents { delta: -1 })
      .await
      .ok();
    return result;
  }

  async fn run_subagent_inner(
    context: &ToolContext<'_>,
    persona: &Persona,
    task: &str,
  ) -> Result<String> {
    let parent_session_id = context.session_id;
    let mut session = Session::new(
      Some(parent_session_id),
      Some(context.model.clone()),
      context.config,
      context.approvals.clone(),
    );
    session.persona_manager.set_persona(persona.clone());
    context.repository.insert_session(&session).await?;
    let session_id = session.id;
    vlog!(
      "Started subagent session {session_id} (parent {parent_session_id}): {task}"
    );

    session
      .ensure_system_message(context.config, context.repository)
      .await?;
    session.add_user_message(task);
    session.persist_last_message(context.repository).await?;

    let result = session
      .harness_loop(
        context.config,
        context.provider_manager,
        context.repository,
        context.tx,
        context.cancel,
      )
      .await;

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
}

fn subagent_turn_limit_note(max_iterations: usize) -> String {
  return format!(
    "\n\n[Subagent stopped after reaching its limit of {max_iterations} turns. Its report is partial and may omit remaining work.]"
  );
}

const CANCELLED_TOOL_RESULT: &str = "Tool call cancelled by the user before completion. Do not retry unless the user asks.";
