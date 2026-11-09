use std::collections::HashMap;

use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::config::secrets::Secrets;
use crate::engine::protocol::{Correlation, SessionEvent};
use crate::engine::repo_actor::RepoHandle;
use crate::persona_manager::Persona;
use crate::provider_manager::{Model, ProviderManager};
use crate::tool_manager::preview::ToolPreview;

pub type ToolParams = HashMap<String, String>;

pub struct ToolContext<'a> {
  pub config: &'a Config,
  pub secrets: &'a Secrets,
  pub persona: &'a Persona,
  pub model: &'a Model,
  pub provider_manager: &'a ProviderManager,
  pub repository: &'a RepoHandle,
  pub events: &'a Sender<SessionEvent>,
  pub correlation: Correlation,
  pub cancel: &'a CancellationToken,
  pub session_id: Uuid,
  pub cwd: &'a str,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolSpec {
  pub name: String,
  pub description: String,
  pub parameters: serde_json::Value,
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
  fn name(&self) -> &str;
  fn description(&self) -> &str;
  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {},
      "required": [],
    });
  }
  async fn execute(
    &self,
    params: ToolParams,
    context: &mut ToolContext<'_>,
  ) -> Result<String>;
  async fn preview(&self, _params: &ToolParams) -> Option<ToolPreview> {
    return None;
  }
  fn is_read_only(&self) -> bool;
  fn reset_cache(&self) {}
  fn to_tool_spec(&self) -> ToolSpec {
    return ToolSpec {
      name: self.name().to_string(),
      description: self.description().to_string(),
      parameters: self.parameters(),
    };
  }
}

pub fn parse_tool_params(arguments: &str) -> ToolParams {
  let mut params = ToolParams::new();
  let value: serde_json::Value =
    serde_json::from_str(arguments).unwrap_or_default();
  if let serde_json::Value::Object(map) = value {
    for (k, v) in map {
      let s = match v {
        serde_json::Value::String(s) => s,
        _ => v.to_string(),
      };
      params.insert(k, s);
    }
  }
  return params;
}
