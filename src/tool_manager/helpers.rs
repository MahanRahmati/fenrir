use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::tool::ToolParams;

pub fn get_param(params: &ToolParams, name: &str) -> ToolManagerResult<String> {
  params
    .get(name)
    .cloned()
    .ok_or(ToolManagerError::MissingParameter {
      param: name.to_string(),
      name: "unknown".to_string(),
    })
}

pub fn get_optional_usize(params: &ToolParams, name: &str) -> Option<usize> {
  params.get(name).and_then(|v| v.parse::<usize>().ok())
}

pub fn get_optional_bool(params: &ToolParams, name: &str) -> Option<bool> {
  params.get(name).and_then(|v| v.parse::<bool>().ok())
}
