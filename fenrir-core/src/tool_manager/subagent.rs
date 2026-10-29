use anyhow::{Context, Result};

use crate::agent_manager::agent::Agent;
use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};

pub const SUBAGENT_TOOL_NAME: &str = "subagent";

pub struct SubagentTool;

#[async_trait::async_trait]
impl Tool for SubagentTool {
  fn name(&self) -> &str {
    return SUBAGENT_TOOL_NAME;
  }

  fn description(&self) -> &str {
    return "Run a task in a subagent that works on its own with the same tools as you and returns a report. Include all relevant context, constraints, and the expected output.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "task": {"type": "string", "description": "The task for the subagent, including all relevant context, constraints, and the expected output"},
      },
      "required": ["task"],
    });
  }

  fn is_read_only(&self) -> bool {
    return false;
  }

  async fn execute(
    &self,
    params: ToolParams,
    context: &mut ToolContext<'_>,
  ) -> Result<String> {
    let task = get_param::<String>(&params, SUBAGENT_TOOL_NAME, "task")?;
    let persona = context.persona.clone();
    return Agent::run_subagent(context, &persona, &task)
      .await
      .context("Failed to run subagent");
  }
}

pub fn task_summary(task: &str) -> String {
  return task
    .lines()
    .find(|line| !line.trim().is_empty())
    .unwrap_or("")
    .trim()
    .to_string();
}
