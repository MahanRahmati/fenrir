mod control_keyboard;
mod datetime;
mod edit_file;
mod errors;
mod find_file;
mod find_text;
mod go;
mod helpers;
mod list_directories;
mod notify;
mod project_context;
mod read_file;
mod remove_file;
mod rust;
mod screenshot;
pub mod tool;
mod write_file;

use std::time::Duration;

use tokio::time;

use crate::config::Config;
use crate::tool_manager::control_keyboard::ControlKeyboardTool;
use crate::tool_manager::datetime::DateTimeTool;
use crate::tool_manager::edit_file::EditFileTool;
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::find_file::FindFileTool;
use crate::tool_manager::find_text::FindTextTool;
use crate::tool_manager::go::GoTool;
use crate::tool_manager::list_directories::ListDirectoriesTool;
use crate::tool_manager::notify::NotifyTool;
use crate::tool_manager::project_context::ProjectContextTool;
use crate::tool_manager::read_file::ReadFileTool;
use crate::tool_manager::remove_file::RemoveFileTool;
use crate::tool_manager::rust::RustTool;
use crate::tool_manager::screenshot::ScreenshotTool;
use crate::tool_manager::tool::{Tool, ToolParams, ToolSpec};
use crate::tool_manager::write_file::WriteFileTool;
use crate::utils::path::is_path_within;

pub struct ToolManager {
  tools: Vec<Box<dyn Tool>>,
  tool_timeout: Duration,
  restrict_write_tools_to_cwd: bool,
}

impl ToolManager {
  pub fn new(config: &Config) -> Self {
    let tool_timeout = Duration::from_secs(config.general.tool_timeout_seconds);
    let restrict_write_tools_to_cwd =
      config.general.restrict_write_tools_to_cwd;
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
    if config.permissions.rust {
      tools.push(Box::new(RustTool));
    }
    if config.permissions.go {
      tools.push(Box::new(GoTool));
    }
    if config.permissions.screenshot {
      tools.push(Box::new(ScreenshotTool));
    }
    if config.permissions.notify {
      tools.push(Box::new(NotifyTool));
    }
    if config.permissions.control_keyboard {
      tools.push(Box::new(ControlKeyboardTool));
    }
    return Self {
      tools,
      tool_timeout,
      restrict_write_tools_to_cwd,
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

  fn find_tool(&self, name: &str) -> Option<&dyn Tool> {
    return self
      .tools
      .iter()
      .find(|t| t.name() == name)
      .map(|t| t.as_ref());
  }

  fn needs_confirmation(&self, name: &str, params: &ToolParams) -> bool {
    let tool = match self.find_tool(name) {
      Some(t) => t,
      None => return false,
    };

    if tool.is_read_only() {
      return false;
    }

    let path = match params.get("path") {
      Some(p) => p,
      None => return false,
    };

    let cwd = match std::env::current_dir() {
      Ok(dir) => dir.canonicalize().unwrap_or(dir),
      Err(_) => return false,
    };

    return !is_path_within(path, &cwd);
  }

  pub fn confirm_if_needed(
    &self,
    name: &str,
    params: &ToolParams,
    confirm: &mut impl FnMut(&str) -> bool,
  ) -> bool {
    if !self.needs_confirmation(name, params) {
      return true;
    }
    if self.restrict_write_tools_to_cwd {
      return false;
    }
    let path = match params.get("path") {
      Some(p) => p.as_str(),
      None => return true,
    };
    let msg = format!("Tool \"{name}\" wants to access: {path}");
    return confirm(&msg);
  }
}
