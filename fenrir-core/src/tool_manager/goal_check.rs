use anyhow::{Context, Result, bail};

use crate::agent_manager::agent::Agent;
use crate::persona_manager::JUDGE_PERSONA;
use crate::tool_manager::helpers::get_param;
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};

pub const GOAL_CHECK_TOOL_NAME: &str = "goal_check";

pub struct GoalCheckTool;

#[async_trait::async_trait]
impl Tool for GoalCheckTool {
  fn name(&self) -> &str {
    return GOAL_CHECK_TOOL_NAME;
  }

  fn description(&self) -> &str {
    return "Check whether a goal is fully achieved. Runs a read-only judge that inspects the result, reports the evidence, and lists any gaps.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "goal": {"type": "string", "description": "The goal to verify, including its acceptance criteria"},
      },
      "required": ["goal"],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  async fn execute(
    &self,
    params: ToolParams,
    context: &mut ToolContext<'_>,
  ) -> Result<String> {
    let goal = get_param::<String>(&params, GOAL_CHECK_TOOL_NAME, "goal")?;
    let goal = goal.trim();
    if goal.is_empty() {
      bail!("Goal cannot be empty");
    }
    let task =
      format!("Determine whether this goal is fully achieved:\n\n{goal}");
    let report = Agent::run_subagent(context, &JUDGE_PERSONA, &task)
      .await
      .context("Failed to check goal")?;
    return Ok(report);
  }
}
