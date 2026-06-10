mod datetime;
pub mod errors;
mod helpers;
mod read_file;
mod tool;
mod write_file;

use crate::tool_manager::datetime::DateTimeTool;
use crate::tool_manager::read_file::ReadFileTool;
use crate::tool_manager::tool::Tool;
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
}
