use tokio::sync::oneshot;

use crate::agent_manager::agent::AgentKind;
use crate::app::AppEvent;
use crate::hook_manager::hook::{
  HookContext, HookDecision, PreToolHook, ToolCallContext,
};
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

    context
      .tx
      .send(AppEvent::ToolApprovalRequest {
        tool_call_id: tool_call.id.to_string(),
        tool_name: tool_call.name.to_string(),
        message: message.clone(),
      })
      .await
      .ok();

    let (reply_tx, reply_rx) = oneshot::channel();
    context
      .tx
      .send(AppEvent::ToolConfirmation {
        message,
        reply: reply_tx,
      })
      .await
      .ok();

    if matches!(reply_rx.await, Ok(true)) {
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
