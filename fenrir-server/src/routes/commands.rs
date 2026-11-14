use axum::extract::Query;
use serde::Deserialize;
use uuid::Uuid;

use fenrir_core::client::api::CommandsData;
use fenrir_core::command::Command;

use crate::api_response::{ApiResult, success};

#[derive(Debug, Default, Deserialize)]
pub struct ListCommandsQuery {
  pub session_id: Option<Uuid>,
}

pub async fn list_commands(
  Query(query): Query<ListCommandsQuery>,
) -> ApiResult<CommandsData> {
  let commands = Command::available_commands(query.session_id.is_some());
  return Ok(success(CommandsData { commands }, "Commands listed"));
}
