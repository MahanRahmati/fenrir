use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::tool::ToolParams;
use crate::vlog;

pub fn get_param<T>(
  params: &ToolParams,
  tool_name: &str,
  param_name: &str,
) -> ToolManagerResult<T>
where
  T: std::str::FromStr,
{
  vlog!(
    "Getting parameter '{}' for tool '{}'",
    param_name,
    tool_name
  );
  let value = params.get(param_name).cloned().ok_or(
    ToolManagerError::MissingParameter {
      param: param_name.to_string(),
      name: tool_name.to_string(),
    },
  )?;
  return value
    .parse::<T>()
    .map_err(|_| ToolManagerError::ExecutionFailed {
      name: tool_name.to_string(),
      reason: format!(
        "Invalid value for parameter '{}': {}",
        param_name, value
      ),
    });
}

pub fn get_optional_param<T>(
  params: &ToolParams,
  tool_name: &str,
  name: &str,
) -> Option<T>
where
  T: std::str::FromStr,
{
  vlog!(
    "Getting optional parameter '{}' for tool '{}'",
    name,
    tool_name
  );
  return params.get(name).and_then(|v| v.parse::<T>().ok());
}
