mod errors;
mod provider;
mod providers;

use crate::config::Config;
use crate::provider_manager::provider::Provider;
use crate::provider_manager::providers::openai_compatible::OpenAICompatibleProvider;

pub struct ProviderManager {
  providers: Vec<Box<dyn Provider>>,
  active_provider: Option<Box<dyn Provider>>,
}

impl ProviderManager {
  pub fn new(config: Config) -> Self {
    let mut providers: Vec<Box<dyn Provider>> = Vec::new();
    if let Some(provider) = setup_openai_compatible(config) {
      providers.push(Box::new(provider));
    }
    return Self {
      providers,
      active_provider: None,
    };
  }
}

fn setup_openai_compatible(config: Config) -> Option<OpenAICompatibleProvider> {
  let config = config.providers.openai_compatible?;
  let url = config.url.as_ref()?;
  if config.models.is_empty() {
    return None;
  }
  return Some(OpenAICompatibleProvider::new(
    url.clone(),
    config.name.clone(),
    config.api_key.clone(),
    config.models.clone(),
  ));
}
