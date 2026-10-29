mod browser;
mod cache;
mod control_keyboard;
mod datetime;
mod ddg;
pub mod edit_file;
mod find_file;
mod find_text;
mod goal_check;
mod helpers;
mod list_directories;
mod music;
mod notify;
mod project_context;
mod read_file;
mod read_pdf;
mod remove_file;
mod screenshot;
mod subagent;
pub mod tool;
pub mod write_file;

use std::time::Duration;

use anyhow::{Result, bail};
use tokio::time;

use crate::agent_manager::agent::AgentKind;
use crate::config::Config;
use crate::persona_manager::ToolAccess;
use crate::tool_manager::browser::BrowserTool;
use crate::tool_manager::control_keyboard::ControlKeyboardTool;
use crate::tool_manager::datetime::DateTimeTool;
use crate::tool_manager::ddg::DdgTool;
use crate::tool_manager::edit_file::EditFileTool;
use crate::tool_manager::find_file::FindFileTool;
use crate::tool_manager::find_text::FindTextTool;
use crate::tool_manager::goal_check::GOAL_CHECK_TOOL_NAME;
use crate::tool_manager::goal_check::GoalCheckTool;
use crate::tool_manager::list_directories::ListDirectoriesTool;
use crate::tool_manager::music::MusicTool;
use crate::tool_manager::notify::NotifyTool;
use crate::tool_manager::project_context::ProjectContextTool;
use crate::tool_manager::read_file::ReadFileTool;
use crate::tool_manager::read_pdf::ReadPdfTool;
use crate::tool_manager::remove_file::RemoveFileTool;
use crate::tool_manager::screenshot::ScreenshotTool;
pub use crate::tool_manager::subagent::SUBAGENT_TOOL_NAME;
use crate::tool_manager::subagent::SubagentTool;
pub use crate::tool_manager::subagent::task_summary;
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams, ToolSpec};
use crate::tool_manager::write_file::WriteFileTool;

pub struct ToolManager {
  tools: Vec<Box<dyn Tool>>,
  tool_timeout: Duration,
}

impl ToolManager {
  pub fn new(config: &Config, agent_kind: &AgentKind) -> Self {
    let tool_timeout = Duration::from_secs(config.general.tool_timeout_seconds);
    let mut tools: Vec<Box<dyn Tool>> = Vec::new();
    if config.permissions.datetime {
      tools.push(Box::new(DateTimeTool));
    }
    if config.permissions.read_file {
      tools.push(Box::new(ReadFileTool::new()));
    }
    if config.permissions.read_pdf {
      tools.push(Box::new(ReadPdfTool));
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
    if config.permissions.screenshot {
      tools.push(Box::new(ScreenshotTool));
    }
    if config.permissions.notify {
      tools.push(Box::new(NotifyTool));
    }
    if config.permissions.control_keyboard {
      tools.push(Box::new(ControlKeyboardTool));
    }
    if config.permissions.music {
      tools.push(Box::new(MusicTool));
    }
    if config.permissions.browser {
      tools.push(Box::new(BrowserTool));
    }
    if config.permissions.ddg {
      tools.push(Box::new(DdgTool::new()));
    }
    let is_primary_agent = matches!(agent_kind, AgentKind::Primary);
    if is_primary_agent && config.subagent.enabled {
      tools.push(Box::new(SubagentTool));
    }
    if is_primary_agent && config.permissions.goal_check {
      tools.push(Box::new(GoalCheckTool));
    }
    return Self {
      tools,
      tool_timeout,
    };
  }

  pub fn get_tools_specs(&self, tool_access: ToolAccess) -> Vec<ToolSpec> {
    return self
      .tools
      .iter()
      .filter(|t| tool_access.allows(t.name()))
      .map(|t| t.to_tool_spec())
      .collect();
  }

  pub async fn execute_tool(
    &self,
    name: &str,
    params: ToolParams,
    context: &mut ToolContext<'_>,
  ) -> Result<String> {
    for tool in &self.tools {
      if tool.name() == name {
        return match name {
          SUBAGENT_TOOL_NAME => {
            Box::pin(SubagentTool::run(params, context)).await
          }
          GOAL_CHECK_TOOL_NAME => {
            Box::pin(GoalCheckTool::run(params, context)).await
          }
          _ => {
            match time::timeout(self.tool_timeout, tool.execute(params)).await {
              Ok(result) => result,
              Err(_) => bail!("Tool '{name}' timed out"),
            }
          }
        };
      }
    }
    bail!("Could not find tool '{name}'");
  }

  pub fn reset_cache(&self) {
    for tool in &self.tools {
      tool.reset_cache();
    }
  }

  fn find_tool(&self, name: &str) -> Option<&dyn Tool> {
    return self
      .tools
      .iter()
      .find(|t| t.name() == name)
      .map(|t| t.as_ref());
  }

  pub fn is_read_only(&self, name: &str) -> Option<bool> {
    return self.find_tool(name).map(|t| t.is_read_only());
  }
}

pub fn runs_subagent(name: &str) -> bool {
  return name == SUBAGENT_TOOL_NAME || name == GOAL_CHECK_TOOL_NAME;
}
