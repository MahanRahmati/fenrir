use anyhow::{Result, bail};

use crate::tool_manager::tool::ToolParams;
use crate::vlog;

pub fn get_param<T>(
  params: &ToolParams,
  tool_name: &str,
  param_name: &str,
) -> Result<T>
where
  T: std::str::FromStr,
{
  vlog!("Getting parameter '{param_name}' for tool '{tool_name}'");
  let Some(value) = params.get(param_name) else {
    bail!("Missing parameter '{param_name}' for tool '{tool_name}'");
  };
  let value = match value.parse::<T>() {
    Ok(value) => value,
    Err(_) => {
      bail!(
        "Failed to execute tool '{tool_name}': invalid value for parameter '{param_name}': {value}"
      );
    }
  };
  return Ok(value);
}

pub fn get_optional_param<T>(
  params: &ToolParams,
  tool_name: &str,
  name: &str,
) -> Option<T>
where
  T: std::str::FromStr,
{
  vlog!("Getting optional parameter '{name}' for tool '{tool_name}'",);
  return params.get(name).and_then(|v| v.parse::<T>().ok());
}
