pub mod errors;
pub mod hook;
mod tool_approval;

use crate::hook_manager::errors::HookManagerResult;
use crate::hook_manager::hook::{Hook, HookContext, HookDecision, HookEvent};
use crate::hook_manager::tool_approval::ToolApprovalHook;

pub struct HookManager {
  hooks: Vec<Box<dyn Hook>>,
}

impl HookManager {
  pub fn new() -> Self {
    let hooks: Vec<Box<dyn Hook>> = vec![Box::new(ToolApprovalHook)];
    return Self { hooks };
  }

  pub async fn run(
    &self,
    event: HookEvent,
    context: &mut HookContext<'_>,
  ) -> HookManagerResult<HookDecision> {
    for hook in &self.hooks {
      if !hook.events().contains(&event) {
        continue;
      }

      match hook.handle(event, context).await? {
        HookDecision::Continue => continue,
        decision => return Ok(decision),
      }
    }

    return Ok(HookDecision::Continue);
  }
}
