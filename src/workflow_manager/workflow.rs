use std::{collections::HashMap, sync::Arc};

use crate::config::Config;
use crate::provider_manager::ProviderManager;
use crate::workflow_manager::errors::WorkflowManagerResult;

pub type WorkflowParams = HashMap<String, String>;

#[async_trait::async_trait]
pub trait Workflow: Send + Sync {
  fn name(&self) -> &str;
  async fn execute(
    &self,
    provider: Arc<ProviderManager>,
    config: Config,
    params: WorkflowParams,
  ) -> WorkflowManagerResult<String>;
}
