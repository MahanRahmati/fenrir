use tokio::sync::broadcast;

use crate::agent_manager::agent::AgentKind;
use crate::app::AppEvent;
use crate::hook_manager::hook::{
  HookContext, HookDecision, PreToolHook, ToolCallContext,
};
use crate::session_manager::{ToolApprovalRequest, ToolApprovalStatus};
use crate::utils::path::is_path_within;
use crate::vlog;

pub struct ToolApprovalHook;

#[async_trait::async_trait]
impl PreToolHook for ToolApprovalHook {
  fn name(&self) -> &str {
    return "tool_approval";
  }

  async fn handle(
    &self,
    context: &HookContext<'_>,
    tool_call: &ToolCallContext<'_>,
  ) -> HookDecision {
    vlog!(
      "Hook '{}' handling PreToolUse for session {}",
      self.name(),
      context.session_id,
    );

    if !context.persona.allowed_tools().allows(tool_call.name) {
      return HookDecision::Deny {
        reason: format!(
          "Tool '{}' is not available for the current persona. Use only the tools available to this persona.",
          tool_call.name
        ),
        error: true,
      };
    }

    let Some(is_read_only) = tool_call.is_read_only else {
      return HookDecision::Allow;
    };

    if is_read_only {
      return HookDecision::Allow;
    }

    let Some(path) = tool_call.params.get("path") else {
      return HookDecision::Allow;
    };

    let cwd = match std::env::current_dir() {
      Ok(dir) => dir.canonicalize().unwrap_or(dir),
      Err(_) => return HookDecision::Allow,
    };

    if is_path_within(path, &cwd) {
      return HookDecision::Allow;
    }

    if context.config.general.restrict_write_tools_to_cwd {
      return HookDecision::Deny {
        reason: format!(
          "Tool '{}' was blocked: the target path '{}' is outside the current working directory. Tools are restricted to the current working directory; retry with a path inside it.",
          tool_call.name, path
        ),
        error: true,
      };
    }

    let message =
      format!("Tool \"{}\" wants to access: {}", tool_call.name, path);
    let message = if matches!(context.agent_kind, AgentKind::Subagent) {
      format!("[subagent] {message}")
    } else {
      message
    };

    let tool_call_id = tool_call.id.to_string();
    let mut decisions = context.approvals.subscribe();

    let request = ToolApprovalRequest::new(
      tool_call_id.clone(),
      context.session_id,
      tool_call.name.to_string(),
      message.clone(),
    );
    if let Err(e) = context
      .session_manager
      .add_pending_tool_approval(&request)
      .await
    {
      vlog!("Failed to record tool approval '{tool_call_id}': {e}");
    }

    let requested = context
      .tx
      .send(AppEvent::ToolApprovalRequest {
        tool_call_id: tool_call_id.clone(),
        tool_name: tool_call.name.to_string(),
        message,
      })
      .await
      .is_ok();

    let status = if !requested {
      ToolApprovalStatus::Denied
    } else {
      let decision = loop {
        tokio::select! {
          biased;
          _ = context.cancel.cancelled() => break None,
          decision = decisions.recv() => match decision {
            Ok(decision) if decision.tool_call_id == tool_call_id => {
              break Some(decision.status);
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => break None,
          },
        }
      };
      match decision {
        Some(ToolApprovalStatus::Approved) => ToolApprovalStatus::Approved,
        _ => ToolApprovalStatus::Denied,
      }
    };

    if let Err(e) = context
      .session_manager
      .resolve_tool_approval(&tool_call_id, status.clone())
      .await
    {
      vlog!("Failed to resolve tool approval '{tool_call_id}': {e}");
    }

    context
      .tx
      .send(AppEvent::ToolApprovalResult {
        tool_call_id: tool_call_id.clone(),
        status: status.clone(),
      })
      .await
      .ok();

    if status == ToolApprovalStatus::Approved {
      return HookDecision::Allow;
    }

    return HookDecision::Deny {
      reason: format!(
        "Tool '{}' was cancelled by the user. Respect the user's decision; do not retry the same action unless the user asks.",
        tool_call.name
      ),
      error: false,
    };
  }
}
