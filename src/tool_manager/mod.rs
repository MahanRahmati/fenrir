mod datetime;
mod edit_file;
mod errors;
mod helpers;
mod read_file;
pub mod tool;
mod write_file;

use crate::config::Config;
use crate::tool_manager::datetime::DateTimeTool;
use crate::tool_manager::edit_file::EditFileTool;
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::read_file::ReadFileTool;
use crate::tool_manager::tool::{Tool, ToolParams, ToolSpec};
use crate::tool_manager::write_file::WriteFileTool;

pub struct ToolManager {
  tools: Vec<Box<dyn Tool>>,
}

impl ToolManager {
  pub fn new(config: Config) -> Self {
    let mut tools: Vec<Box<dyn Tool>> = Vec::new();

    if config.permissions.datetime {
      tools.push(Box::new(DateTimeTool));
    }
    if config.permissions.read_file {
      tools.push(Box::new(ReadFileTool));
    }
    if config.permissions.write_file {
      tools.push(Box::new(WriteFileTool));
    }
    if config.permissions.edit_file {
      tools.push(Box::new(EditFileTool));
    }
    return Self { tools };
  }

  pub fn get_tools_specs(&self) -> Vec<ToolSpec> {
    return self.tools.iter().map(|t| t.to_tool_spec()).collect();
  }

  pub fn execute_tool(
    &self,
    name: &str,
    params: ToolParams,
  ) -> ToolManagerResult<String> {
    for tool in &self.tools {
      if tool.name() == name {
        return tool.execute(params);
      }
    }
    return Err(ToolManagerError::NotFound {
      name: name.to_string(),
    });
  }
}
