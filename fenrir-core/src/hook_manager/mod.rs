mod go;
mod helpers;
pub mod hook;
mod rust;
mod tool_approval;

use crate::config::Config;
use crate::hook_manager::go::GoHook;
use crate::hook_manager::hook::{
  HookContext, HookDecision, PostToolHook, PreToolHook, ToolCallContext,
  ToolResultContext,
};
use crate::hook_manager::rust::RustHook;
use crate::hook_manager::tool_approval::ToolApprovalHook;

pub struct HookManager {
  pre_tool_use_hooks: Vec<Box<dyn PreToolHook>>,
  post_tool_use_hooks: Vec<Box<dyn PostToolHook>>,
}

impl HookManager {
  pub fn new(config: &Config) -> Self {
    let pre_tool_use_hooks: Vec<Box<dyn PreToolHook>> =
      vec![Box::new(ToolApprovalHook)];
    let mut post_tool_use_hooks: Vec<Box<dyn PostToolHook>> = Vec::new();
    if config.hooks.rust {
      post_tool_use_hooks.push(Box::new(RustHook));
    }
    if config.hooks.go {
      post_tool_use_hooks.push(Box::new(GoHook));
    }
    return Self {
      pre_tool_use_hooks,
      post_tool_use_hooks,
    };
  }

  pub async fn pre_tool_use(
    &self,
    context: &HookContext<'_>,
    tool_call: &ToolCallContext<'_>,
  ) -> HookDecision {
    for hook in &self.pre_tool_use_hooks {
      match hook.handle(context, tool_call).await {
        HookDecision::Continue => continue,
        decision => return decision,
      }
    }
    return HookDecision::Continue;
  }

  pub async fn post_tool_use(
    &self,
    context: &HookContext<'_>,
    tool_result: &ToolResultContext<'_>,
  ) -> String {
    let mut failure = String::new();
    for hook in &self.post_tool_use_hooks {
      failure.push_str(&hook.handle(context, tool_result).await);
    }
    return failure;
  }
}
