use anyhow::{Context, Result};

use crate::provider_manager::Model;
use crate::session_manager::session::Session;
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
    return "Run a task in a subagent that works on its own with the same tools as you and returns a report. Include all relevant context, constraints, and the expected output. Set complexity to 'hard' when the task needs deep reasoning, such as complex refactors, subtle debugging, architecture, or careful analysis.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "task": {"type": "string", "description": "The task for the subagent, including all relevant context, constraints, and the expected output"},
        "complexity": {
          "type": "string",
          "enum": ["normal", "hard"],
          "description": "Use 'hard' for tasks that need deep reasoning: complex refactors, subtle debugging, architecture, or careful analysis. Defaults to 'normal'."
        },
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
    let model = task_model(context, &params);
    return Session::run_subagent(context, &persona, &task, &model)
      .await
      .context("Failed to run subagent");
  }
}

fn task_model(context: &ToolContext<'_>, params: &ToolParams) -> Model {
  let is_hard =
    get_optional_param::<String>(params, SUBAGENT_TOOL_NAME, "complexity")
      .as_deref()
      == Some("hard");
  if is_hard {
    return context
      .provider_manager
      .large_model()
      .unwrap_or_else(|| context.model.clone());
  }
  return context.model.clone();
}
