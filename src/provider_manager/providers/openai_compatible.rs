use crate::config::ProviderModel;
use crate::provider_manager::provider::{Model, Provider};
use crate::vlog;

pub struct OpenAICompatibleProvider {
  name: String,
  api_key: Option<String>,
  models: Vec<Model>,
  active_model: Option<Model>,
}

impl OpenAICompatibleProvider {
  pub fn new(
    url: String,
    name: Option<String>,
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
      name: name.unwrap_or("OpenAI-compatible".to_string()),
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

  fn select_model(&mut self, model: &Model) {
    vlog!("Switching active model to: {}", model.name);
    self.active_model = Some(model.clone());
  }
}
