use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::tool::ToolParams;
use crate::vlog;

pub fn get_param(
  params: &ToolParams,
  tool_name: &str,
  param_name: &str,
) -> ToolManagerResult<String> {
  vlog!(
    "Getting parameter '{}' for tool '{}'",
    param_name,
    tool_name
  );
  return params.get(param_name).cloned().ok_or(
    ToolManagerError::MissingParameter {
      param: param_name.to_string(),
      name: tool_name.to_string(),
    },
  );
}

pub fn get_optional_param(
  params: &ToolParams,
  tool_name: &str,
  name: &str,
) -> Option<String> {
  vlog!(
    "Getting optional parameter '{}' for tool '{}'",
    name,
    tool_name
  );
  return params.get(name).cloned();
}

pub fn get_optional_usize(
  params: &ToolParams,
  tool_name: &str,
  name: &str,
) -> Option<usize> {
  vlog!(
    "Getting optional usize parameter '{}' for tool '{}'",
    name,
    tool_name
  );
  return params.get(name).and_then(|v| v.parse::<usize>().ok());
}

pub fn get_optional_bool(
  params: &ToolParams,
  tool_name: &str,
  name: &str,
) -> Option<bool> {
  vlog!(
    "Getting optional usize parameter '{}' for tool '{}'",
    name,
    tool_name
  );
  return params.get(name).and_then(|v| v.parse::<bool>().ok());
}
