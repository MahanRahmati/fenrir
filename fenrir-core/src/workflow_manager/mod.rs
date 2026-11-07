pub mod prediction;

use anyhow::Result;

use crate::config::Config;
use crate::provider_manager::{Model, ProviderManager};
use crate::workflow_manager::prediction::{
  PredictionParams, PredictionWorkflow,
};

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
