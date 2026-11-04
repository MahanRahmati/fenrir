pub mod compaction;
pub mod prediction;
pub mod title;

use anyhow::Result;

use crate::config::Config;
use crate::provider_manager::{Model, ProviderManager};
use crate::workflow_manager::compaction::{
  CompactionParams, CompactionWorkflow,
};
use crate::workflow_manager::prediction::{
  PredictionParams, PredictionWorkflow,
};
use crate::workflow_manager::title::{TitleParams, TitleWorkflow};

pub struct WorkflowManager {
  config: Config,
  cwd: String,
}

impl WorkflowManager {
  pub fn new(config: &Config, cwd: &str) -> Self {
    return Self {
      config: config.clone(),
      cwd: cwd.to_owned(),
    };
  }

  pub async fn generate_title(
    &self,
    providers: &ProviderManager,
    model: &Model,
    params: TitleParams,
  ) -> Result<String> {
    return TitleWorkflow::execute(
      providers,
      &self.config,
      model,
      params,
      &self.cwd,
    )
    .await;
  }

  pub async fn compact(
    &self,
    providers: &ProviderManager,
    model: &Model,
    params: CompactionParams,
  ) -> Result<String> {
    return CompactionWorkflow::execute(
      providers,
      &self.config,
      model,
      params,
      &self.cwd,
    )
    .await;
  }

  pub async fn predict_next_prompt(
    &self,
    providers: &ProviderManager,
    model: &Model,
    params: PredictionParams,
  ) -> Result<String> {
    return PredictionWorkflow::execute(
      providers,
      &self.config,
      model,
      params,
      &self.cwd,
    )
    .await;
  }
}
