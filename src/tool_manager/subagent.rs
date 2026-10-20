use crate::agent_manager::agent::Agent;
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
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
    return "Run a task in a subagent that works on its own and returns a report. Include all relevant context, constraints, and the expected output.";
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

  async fn execute(&self, _params: ToolParams) -> ToolManagerResult<String> {
    return Err(ToolManagerError::ExecutionFailed {
      name: SUBAGENT_TOOL_NAME.to_string(),
      reason: "The subagent tool requires an execution context".to_string(),
    });
  }
}

impl SubagentTool {
  pub async fn run(
    params: ToolParams,
    context: &mut ToolContext<'_>,
  ) -> ToolManagerResult<String> {
    let task = get_param::<String>(&params, SUBAGENT_TOOL_NAME, "task")?;
    return Agent::run_subagent(context, &task).await.map_err(|e| {
      ToolManagerError::ExecutionFailed {
        name: SUBAGENT_TOOL_NAME.to_string(),
        reason: e.to_string(),
      }
    });
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
