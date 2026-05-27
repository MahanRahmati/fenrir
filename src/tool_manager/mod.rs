mod time;
mod tool;

use crate::tool_manager::time::TimeTool;
use crate::tool_manager::tool::Tool;

pub struct ToolManager {
  tools: Vec<Box<dyn Tool>>,
}

impl ToolManager {
  pub fn new() -> Self {
    let tools: Vec<Box<dyn Tool>> = vec![Box::new(TimeTool)];
    return Self { tools };
  }

  pub fn get_tools(&self) -> &Vec<Box<dyn Tool>> {
    return &self.tools;
  }
}
