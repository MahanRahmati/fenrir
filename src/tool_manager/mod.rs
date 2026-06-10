mod datetime;
pub mod errors;
mod tool;

use std::collections::HashMap;

use crate::tool_manager::datetime::DateTimeTool;
use crate::tool_manager::tool::Tool;

pub type ToolParams = HashMap<String, String>;

pub struct ToolManager {
  tools: Vec<Box<dyn Tool>>,
}

impl ToolManager {
  pub fn new() -> Self {
    let tools: Vec<Box<dyn Tool>> = vec![Box::new(DateTimeTool)];
    return Self { tools };
  }

  pub fn get_tools(&self) -> &Vec<Box<dyn Tool>> {
    return &self.tools;
  }
}
