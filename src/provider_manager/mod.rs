mod errors;
mod provider;
mod providers;

use crate::config::Config;
use crate::provider_manager::errors::{
  ProviderManagerError, ProviderManagerResult,
};
pub use crate::provider_manager::provider::ChatUsage;
pub use crate::provider_manager::provider::Model;
pub use crate::provider_manager::provider::ToolCall;
use crate::provider_manager::provider::{ChatResponse, ChatStream, Provider};
use crate::provider_manager::providers::openai_compatible::OpenAICompatibleProvider;
use crate::session_manager::Message;
use crate::tool_manager::tool::ToolSpec;
use crate::vlog;

pub struct ModelInfo {
  pub provider: String,
  pub model: Model,
  pub active: bool,
}

pub struct ProviderManager {
  providers: Vec<Box<dyn Provider>>,
  active_provider: Option<usize>,
  active_model: Option<String>,
}

impl ProviderManager {
  pub fn new(config: Config) -> Self {
    let mut providers: Vec<Box<dyn Provider>> = Vec::new();
    if let Some(provider) = setup_openai_compatible(config.clone()) {
      providers.push(Box::new(provider));
    }

    let active_provider = setup_active_provider(config.clone(), &providers);
    let mut active_model = None;
    if let Some(idx) = active_provider {
      let provider = &mut providers[idx];
      let model_str = config.general.model.unwrap_or_default();
      let (_, model_name) = model_str
        .split_once('/')
        .unwrap_or(("", model_str.as_str()));
      let found_model = provider
        .list_models()
        .into_iter()
        .find(|m| m.name == model_name);
      if let Some(model) = found_model {
        active_model = Some(model.name.clone());
        provider.set_active_model(model);
      }
    }

    return Self {
      providers,
      active_provider,
      active_model,
    };
  }

  pub fn list_models(&self) -> Vec<ModelInfo> {
    let mut models = Vec::new();
    for (index, provider) in self.providers.iter().enumerate() {
      let is_active_provider = self.active_provider == Some(index);
      for model in provider.list_models() {
        let active = is_active_provider
          && self.active_model.as_deref() == Some(model.name.as_str());
        models.push(ModelInfo {
          provider: provider.name().to_string(),
          model,
          active,
        });
      }
    }
    return models;
  }

  pub async fn chat(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> ProviderManagerResult<ChatResponse> {
    let provider = self
      .active_provider
      .and_then(|i| self.providers.get(i))
      .ok_or(ProviderManagerError::NoActiveProvider)?;
    return provider.chat(messages, tools).await;
  }

  pub async fn chat_stream(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> ProviderManagerResult<ChatStream> {
    let provider = self
      .active_provider
      .and_then(|i| self.providers.get(i))
      .ok_or(ProviderManagerError::NoActiveProvider)?;
    return provider.chat_stream(messages, tools).await;
  }
}

fn setup_active_provider(
  config: Config,
  providers: &Vec<Box<dyn Provider>>,
) -> Option<usize> {
  let model_str = config.general.model?;
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
