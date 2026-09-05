mod errors;
mod title;
pub mod workflow;

use std::sync::Arc;

use crate::config::Config;
use crate::provider_manager::ProviderManager;
use crate::workflow_manager::errors::{
  WorkflowManagerError, WorkflowManagerResult,
};
use crate::workflow_manager::title::TitleWorkflow;
use crate::workflow_manager::workflow::{Workflow, WorkflowParams};

pub struct WorkflowManager {
  workflows: Vec<Box<dyn Workflow>>,
  provider: Arc<ProviderManager>,
  config: Config,
}

impl WorkflowManager {
  pub fn new(config: Config, provider: Arc<ProviderManager>) -> Self {
    let mut workflows: Vec<Box<dyn Workflow>> = Vec::new();
    workflows.push(Box::new(TitleWorkflow));
    return Self {
      workflows,
      provider,
      config,
    };
  }

  pub async fn execute_workflow(
    &self,
    name: &str,
    params: WorkflowParams,
  ) -> WorkflowManagerResult<String> {
    for workflow in &self.workflows {
      if workflow.name() == name {
        return workflow
          .execute(self.provider.clone(), self.config.clone(), params)
          .await;
      }
    }
    return Err(WorkflowManagerError::NotFound {
      name: name.to_string(),
    });
  }
}
