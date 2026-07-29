mod datetime;
mod edit_file;
mod errors;
mod find_file;
mod find_text;
mod helpers;
mod list_directories;
mod project_context;
mod read_file;
mod remove_file;
pub mod tool;
mod write_file;

use std::time::Duration;

use tokio::time;

use crate::config::Config;
use crate::tool_manager::datetime::DateTimeTool;
use crate::tool_manager::edit_file::EditFileTool;
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::find_file::FindFileTool;
use crate::tool_manager::find_text::FindTextTool;
use crate::tool_manager::list_directories::ListDirectoriesTool;
use crate::tool_manager::project_context::ProjectContextTool;
use crate::tool_manager::read_file::ReadFileTool;
use crate::tool_manager::remove_file::RemoveFileTool;
use crate::tool_manager::tool::{Tool, ToolParams, ToolSpec};
use crate::tool_manager::write_file::WriteFileTool;

pub struct ToolManager {
  tools: Vec<Box<dyn Tool>>,
  tool_timeout: Duration,
}

impl ToolManager {
  pub fn new(config: Config) -> Self {
    let tool_timeout = Duration::from_secs(config.general.tool_timeout_seconds);
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
    if config.permissions.remove_file {
      tools.push(Box::new(RemoveFileTool));
    }
    if config.permissions.find_file {
      tools.push(Box::new(FindFileTool));
    }
    if config.permissions.find_text {
      tools.push(Box::new(FindTextTool));
    }
    if config.permissions.list_directories {
      tools.push(Box::new(ListDirectoriesTool));
    }
    if config.permissions.project_context {
      tools.push(Box::new(ProjectContextTool));
    }
    return Self {
      tools,
      tool_timeout,
    };
  }

  pub fn get_tools_specs(&self) -> Vec<ToolSpec> {
    return self.tools.iter().map(|t| t.to_tool_spec()).collect();
  }

  pub async fn execute_tool(
    &self,
    name: &str,
    params: ToolParams,
  ) -> ToolManagerResult<String> {
    for tool in &self.tools {
      if tool.name() == name {
        match time::timeout(self.tool_timeout, tool.execute(params)).await {
          Ok(result) => return result,
          Err(_) => {
            return Err(ToolManagerError::Timeout {
              name: name.to_string(),
            });
          }
        }
      }
    }
    return Err(ToolManagerError::NotFound {
      name: name.to_string(),
    });
  }
}
