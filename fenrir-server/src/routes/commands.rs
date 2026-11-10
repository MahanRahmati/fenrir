use serde::Serialize;

use fenrir_core::command::Command;
use fenrir_core::engine::protocol::CommandInfo;

use crate::api_response::{ApiResponse, ApiResult};

#[derive(Debug, Serialize)]
pub struct CommandsData {
  pub commands: Vec<CommandInfo>,
}

pub async fn list_commands() -> ApiResult<CommandsData> {
  return Ok(ApiResponse::success(
    CommandsData {
      commands: Command::catalog(),
    },
    "Commands listed",
  ));
}
