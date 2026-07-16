use std::collections::HashMap;

use crate::tool_manager::errors::ToolManagerResult;

pub type ToolParams = HashMap<String, String>;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolSpec {
  pub name: String,
  pub description: String,
  pub parameters: serde_json::Value,
}

pub trait Tool {
  fn name(&self) -> &str;
  fn description(&self) -> &str;
  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {},
      "required": [],
    });
  }
  fn execute(&self, params: ToolParams) -> ToolManagerResult<String>;
  fn to_tool_spec(&self) -> ToolSpec {
    return ToolSpec {
      name: self.name().to_string(),
      description: self.description().to_string(),
      parameters: self.parameters(),
    };
  }
}
