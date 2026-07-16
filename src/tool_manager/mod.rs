mod datetime;
mod errors;
mod helpers;
mod read_file;
pub mod tool;
mod write_file;

use crate::tool_manager::datetime::DateTimeTool;
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::read_file::ReadFileTool;
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::tool_manager::write_file::WriteFileTool;

pub struct ToolManager {
  tools: Vec<Box<dyn Tool>>,
}

impl ToolManager {
  pub fn new() -> Self {
    let tools: Vec<Box<dyn Tool>> = vec![
      Box::new(DateTimeTool),
      Box::new(ReadFileTool),
      Box::new(WriteFileTool),
    ];
    return Self { tools };
  }

  pub fn get_tools(&self) -> &Vec<Box<dyn Tool>> {
    return &self.tools;
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
