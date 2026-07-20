mod errors;
mod provider;
mod providers;

use crate::config::Config;
use crate::provider_manager::errors::{
  ProviderManagerError, ProviderManagerResult,
};
use crate::provider_manager::provider::{ChatResponse, Provider};
use crate::provider_manager::providers::openai_compatible::OpenAICompatibleProvider;
use crate::session_manager::Message;
use crate::tool_manager::tool::ToolSpec;
use crate::vlog;

pub struct ProviderManager {
  providers: Vec<Box<dyn Provider>>,
  active_provider: Option<usize>,
}

impl ProviderManager {
  pub fn new(config: Config) -> Self {
    let mut providers: Vec<Box<dyn Provider>> = Vec::new();
    if let Some(provider) = setup_openai_compatible(config.clone()) {
      providers.push(Box::new(provider));
    }
    let active_provider = setup_active_provider(config, &providers);
    return Self {
      providers,
      active_provider,
    };
  }

  pub fn chat(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> ProviderManagerResult<ChatResponse> {
    let provider = self
      .active_provider
      .and_then(|i| self.providers.get(i))
      .ok_or_else(|| ProviderManagerError::NoActiveProvider)?;
    return provider.chat(messages, tools);
  }
}

fn setup_active_provider(
  config: Config,
  providers: &Vec<Box<dyn Provider>>,
) -> Option<usize> {
  let model_str = config.general.model.clone()?;
  let (provider_name, model_name) = model_str.split_once('/')?;

  for (i, p) in providers.iter().enumerate() {
    if p.name() != provider_name {
      continue;
    }

    let has_model = p.list_models().iter().any(|m| m.name == model_name);

    if has_model {
      vlog!("Selected provider '{provider_name}' model '{model_name}'");
      return Some(i);
    }
  }

  vlog!("Failed to select provider '{provider_name}' model '{model_name}'");
  return None;
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
