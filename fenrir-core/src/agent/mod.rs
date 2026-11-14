mod turn;

use anyhow::{Result, bail};
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

pub use turn::{run_compact, run_turn};

use crate::config::Config;
use crate::config::secrets::Secrets;
use crate::engine::protocol::{Correlation, ServerEvent, SessionEvent};
use crate::engine::repo_actor::RepoHandle;
use crate::hook_manager::hook::{
  HookContext, HookDecision, ToolCallContext, ToolResultContext,
};
use crate::persona_manager::Persona;
use crate::prompt::{PromptExecutor, chat};
use crate::provider_manager::{Model, ProviderManager, ToolCall};
use crate::session_manager::session::Session;
use crate::tool_manager::SUBAGENT_TOOL_NAME;
use crate::tool_manager::runs_subagent;
use crate::tool_manager::tool::{ToolContext, parse_tool_params};
use crate::vlog;

pub struct AgentContext<'a> {
  pub config: &'a Config,
  pub secrets: &'a Secrets,
  pub provider_manager: &'a ProviderManager,
  pub repo: &'a RepoHandle,
  pub events: &'a Sender<SessionEvent>,
  pub cancel: &'a CancellationToken,
}

pub async fn run_agent_loop(
  session: &mut Session,
  context: &AgentContext<'_>,
  correlation: Correlation,
) -> Result<String> {
  let Some(model) = session.model.clone() else {
    bail!("No model selected");
  };
  let is_subagent = session.is_subagent();
  let max_iterations = context.config.subagent.max_iterations;
  let executor = PromptExecutor::new(context.provider_manager, context.config);
  let system = chat::system_prompt(session, context.config);
  let mut turns = 0usize;
  let mut assistant = String::new();

  loop {
    if context.cancel.is_cancelled() {
      break;
    }
    if is_subagent && turns >= max_iterations {
      assistant.push_str(&turn_limit_note(max_iterations));
      break;
    }
    turns += 1;

    let mut spec = chat::build(session, &model, &system);
    spec.emit_tokens = !is_subagent;
    let output = executor
      .once(
        spec,
        Some(session.tool_manager.skills()),
        correlation,
        context.events,
        context.cancel,
      )
      .await?;

    if let Some(usage) = output.usage {
      session.last_usage = Some(usage);
    }

    if context.config.general.show_reasoning
      && let Some(reasoning) = output.reasoning.as_deref()
      && !reasoning.is_empty()
    {
      session.add_reasoning_message(reasoning);
      session.persist_last_message(context.repo).await?;
    }

    if let Some(content) = output.content.as_deref()
      && !content.is_empty()
    {
      assistant.push_str(content);
      session.add_assistant_message(content);
      session.persist_last_message(context.repo).await?;
    }

    if output.tool_calls.is_empty() {
      break;
    }

    execute_tool_calls(
      session,
      context,
      correlation,
      &model,
      &output.tool_calls,
    )
    .await?;
  }

  return Ok(assistant);
}

async fn execute_tool_calls(
  session: &mut Session,
  context: &AgentContext<'_>,
  correlation: Correlation,
  model: &Model,
  tool_calls: &[ToolCall],
) -> Result<()> {
  let is_subagent = session.is_subagent();

  for tool_call in tool_calls {
    if context.cancel.is_cancelled() {
      break;
    }
    vlog!(
      "Tool call: {} with arguments: {}",
      tool_call.name,
      tool_call.arguments
    );

    let preview = session
      .tool_manager
      .preview_tool(&tool_call.name, &tool_call.arguments)
      .await;

    session.add_tool_call(
      &tool_call.id,
      &tool_call.name,
      &tool_call.arguments,
      preview.clone(),
    );
    session.persist_last_message(context.repo).await?;

    if context.cancel.is_cancelled() {
      session.add_tool_result_message(&tool_call.id, CANCELLED_TOOL_RESULT);
      session.persist_last_message(context.repo).await?;
      break;
    }

    let params = parse_tool_params(&tool_call.arguments);
    let is_subagent_tool = tool_call.name == SUBAGENT_TOOL_NAME;

    if context.config.general.show_tools && !is_subagent {
      let _ = context
        .events
        .send(SessionEvent::Event(ServerEvent::ToolCall {
          correlation,
          id: tool_call.id.clone(),
          name: tool_call.name.clone(),
          arguments: tool_call.arguments.clone(),
          preview,
        }))
        .await;
    }

    let hook = HookContext {
      session_id: session.id,
      correlation,
      config: context.config,
      persona: session.persona_manager.current(),
      is_subagent,
      events: context.events,
      repository: context.repo,
      cancel: context.cancel,
      cwd: session.cwd.as_str(),
    };
    let call = ToolCallContext {
      id: &tool_call.id,
      name: &tool_call.name,
      params: &params,
      is_read_only: session.tool_manager.is_read_only(&tool_call.name),
    };

    match session.hook_manager.pre_tool_use(&hook, &call).await {
      HookDecision::Continue | HookDecision::Allow => {}
      HookDecision::Deny { reason, error } => {
        session.add_tool_result_message(&tool_call.id, &reason);
        session.persist_last_message(context.repo).await?;
        if context.config.general.show_tools && !is_subagent {
          let _ = context
            .events
            .send(SessionEvent::Event(ServerEvent::ToolResult {
              correlation,
              id: tool_call.id.clone(),
              result: reason,
              error,
            }))
            .await;
        }
        continue;
      }
    }

    let outcome = {
      let mut tool_context = ToolContext {
        config: context.config,
        secrets: context.secrets,
        skills: session.tool_manager.skills(),
        persona: session.persona_manager.current(),
        model,
        provider_manager: context.provider_manager,
        repository: context.repo,
        events: context.events,
        correlation,
        cancel: context.cancel,
        session_id: session.id,
        cwd: session.cwd.as_str(),
      };
      if runs_subagent(&tool_call.name) {
        Some(
          session
            .tool_manager
            .execute_tool(&tool_call.name, params, &mut tool_context)
            .await,
        )
      } else {
        tokio::select! {
          biased;
          _ = context.cancel.cancelled() => None,
          result = session.tool_manager.execute_tool(
            &tool_call.name,
            params,
            &mut tool_context,
          ) => Some(result),
        }
      }
    };

    let Some(result) = outcome else {
      session.add_tool_result_message(&tool_call.id, CANCELLED_TOOL_RESULT);
      session.persist_last_message(context.repo).await?;
      break;
    };

    let (mut result_str, error) = match result {
      Ok(result) => {
        vlog!("Tool '{}' result: {}", tool_call.name, result);
        (result, false)
      }
      Err(e) => {
        vlog!("Tool '{}' error: {}", tool_call.name, e);
        (format!("Tool error: {e}"), true)
      }
    };

    let result_context = ToolResultContext {
      name: &tool_call.name,
      arguments: &tool_call.arguments,
      error: &error,
    };
    let hook = HookContext {
      session_id: session.id,
      correlation,
      config: context.config,
      persona: session.persona_manager.current(),
      is_subagent,
      events: context.events,
      repository: context.repo,
      cancel: context.cancel,
      cwd: session.cwd.as_str(),
    };
    let failure = session
      .hook_manager
      .post_tool_use(&hook, &result_context)
      .await;

    if !failure.is_empty() {
      vlog!(
        "Post tool use hook failure for '{}': {}",
        tool_call.name,
        failure
      );
      result_str.push_str(&failure);
    }

    session.add_tool_result_message(&tool_call.id, &result_str);
    session.persist_last_message(context.repo).await?;

    if context.config.general.show_tools
      && !is_subagent
      && (!is_subagent_tool || error)
    {
      let _ = context
        .events
        .send(SessionEvent::Event(ServerEvent::ToolResult {
          correlation,
          id: tool_call.id.clone(),
          result: result_str,
          error,
        }))
        .await;
    }
  }

  return Ok(());
}

impl Session {
  pub async fn run_subagent(
    context: &ToolContext<'_>,
    persona: &Persona,
    task: &str,
    model: &Model,
  ) -> Result<String> {
    let task = task.trim();
    if task.is_empty() {
      bail!("Task cannot be empty");
    }

    let mut session = context
      .repository
      .create_child_session(
        Some(context.session_id),
        Some(persona.name().to_string()),
        Some(model.clone()),
        context.cwd.to_string(),
      )
      .await?;
    let session_id = session.id;
    vlog!(
      "Started subagent session {session_id} (parent {}): {task}",
      context.session_id
    );

    session
      .ensure_system_message_with(context.config, context.repository)
      .await?;
    session.add_user_message(task);
    session.persist_last_message(context.repository).await?;

    let _ = context
      .events
      .send(SessionEvent::Event(ServerEvent::Subagents {
        correlation: context.correlation,
        delta: 1,
      }))
      .await;

    let agent_context = AgentContext {
      config: context.config,
      secrets: context.secrets,
      provider_manager: context.provider_manager,
      repo: context.repository,
      events: context.events,
      cancel: context.cancel,
    };
    let result =
      run_agent_loop(&mut session, &agent_context, context.correlation).await;

    let _ = context
      .events
      .send(SessionEvent::Event(ServerEvent::Subagents {
        correlation: context.correlation,
        delta: -1,
      }))
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

fn turn_limit_note(max_iterations: usize) -> String {
  return format!(
    "\n\n[Subagent stopped after reaching its limit of {max_iterations} turns. Its report is partial and may omit remaining work.]"
  );
}

const CANCELLED_TOOL_RESULT: &str = "Tool call cancelled by the user before completion. Do not retry unless the user asks.";
