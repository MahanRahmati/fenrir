pub mod compaction;
pub mod errors;
pub mod title;

use std::sync::Arc;

use crate::config::Config;
use crate::provider_manager::ProviderManager;
use crate::workflow_manager::compaction::{
  CompactionParams, CompactionWorkflow,
};
use crate::workflow_manager::errors::WorkflowManagerResult;
use crate::workflow_manager::title::{TitleParams, TitleWorkflow};

pub struct WorkflowManager {
  provider: Arc<ProviderManager>,
  config: Config,
}

impl WorkflowManager {
  pub fn new(config: Config, provider: Arc<ProviderManager>) -> Self {
    return Self { provider, config };
  }

  pub async fn generate_title(
    &self,
    params: TitleParams,
  ) -> WorkflowManagerResult<String> {
    return TitleWorkflow::execute(
      self.provider.clone(),
      self.config.clone(),
      params,
    )
    .await;
  }

  pub async fn compact(
    &self,
    params: CompactionParams,
  ) -> WorkflowManagerResult<String> {
    return CompactionWorkflow::execute(
      self.provider.clone(),
      self.config.clone(),
      params,
    )
    .await;
  }
}
