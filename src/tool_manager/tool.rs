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

pub fn parse_tool_params(arguments: &str) -> ToolParams {
  let mut params = ToolParams::new();
  let value: serde_json::Value =
    serde_json::from_str(arguments).unwrap_or_default();
  if let serde_json::Value::Object(map) = value {
    for (k, v) in map {
      let s = match v {
        serde_json::Value::String(s) => s.clone(),
        _ => v.to_string(),
      };
      params.insert(k, s);
    }
  }
  return params;
}
