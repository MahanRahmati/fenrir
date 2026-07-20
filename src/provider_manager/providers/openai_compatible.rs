use crate::config::ProviderModel;
use crate::provider_manager::provider::{Model, Provider};
use crate::vlog;

#[derive(Clone)]
pub struct OpenAICompatibleProvider {
  name: String,
  api_key: Option<String>,
  models: Vec<Model>,
  active_model: Option<Model>,
}

impl OpenAICompatibleProvider {
  pub fn new(
    url: String,
    name: String,
    api_key: Option<String>,
    models: Vec<ProviderModel>,
  ) -> Self {
    let models = models
      .into_iter()
      .map(|m| Model {
        name: m.name,
        alias: m.alias,
      })
      .collect();
    return Self {
      name: name,
      api_key,
      models,
      active_model: None,
    };
  }
}

impl Provider for OpenAICompatibleProvider {
  fn name(&self) -> &str {
    return &self.name;
  }

  fn active_model(&self) -> Option<&Model> {
    return self.active_model.as_ref();
  }

  fn list_models(&self) -> Vec<Model> {
    return self
      .models
      .iter()
      .map(|m| Model {
        name: m.name.clone(),
        alias: m.alias.clone(),
      })
      .collect();
  }
}
