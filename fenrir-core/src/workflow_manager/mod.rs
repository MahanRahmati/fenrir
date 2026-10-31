pub mod compaction;
pub mod title;

use anyhow::Result;

use crate::config::Config;
use crate::provider_manager::{Model, ProviderManager};
use crate::workflow_manager::compaction::{
  CompactionParams, CompactionWorkflow,
};
use crate::workflow_manager::title::{TitleParams, TitleWorkflow};

pub struct WorkflowManager {
  config: Config,
}

impl WorkflowManager {
  pub fn new(config: Config) -> Self {
    return Self { config };
  }

  pub async fn generate_title(
    &self,
    providers: &ProviderManager,
    model: &Model,
    params: TitleParams,
  ) -> Result<String> {
    return TitleWorkflow::execute(providers, &self.config, model, params)
      .await;
  }

  pub async fn compact(
    &self,
    providers: &ProviderManager,
    model: &Model,
    params: CompactionParams,
  ) -> Result<String> {
    return CompactionWorkflow::execute(providers, &self.config, model, params)
      .await;
  }
}
