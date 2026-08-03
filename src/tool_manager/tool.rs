use std::collections::HashMap;

use crate::tool_manager::errors::ToolManagerResult;

pub type ToolParams = HashMap<String, String>;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolSpec {
  pub name: String,
  pub description: String,
  pub parameters: serde_json::Value,
}

#[async_trait::async_trait]
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
  async fn execute(&self, params: ToolParams) -> ToolManagerResult<String>;
  fn is_read_only(&self) -> bool;
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
        serde_json::Value::String(s) => s,
        _ => v.to_string(),
      };
      params.insert(k, s);
    }
  }
  return params;
}
