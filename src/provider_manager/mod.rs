mod errors;
mod provider;
mod providers;

use std::sync::Mutex;

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

pub struct ProviderManager {
  providers: Vec<Box<dyn Provider>>,
  active_model: Mutex<Option<Model>>,
}

impl ProviderManager {
  pub fn new(config: Config) -> Self {
    let mut providers: Vec<Box<dyn Provider>> = Vec::new();
    if let Some(provider) = setup_openai_compatible(config.clone()) {
      providers.push(Box::new(provider));
    }

    let active_model = resolve_active_model(&config, &providers);
    return Self {
      providers,
      active_model: Mutex::new(active_model),
    };
  }

  pub fn list_models(&self) -> Vec<Model> {
    let mut models = Vec::new();
    for provider in self.providers.iter() {
      models.extend(provider.list_models());
    }
    return models;
  }

  pub fn active_model(&self) -> Option<Model> {
    return self.active_model.lock().unwrap().clone();
  }

  pub fn set_active_model(&self, model: &Model) {
    *self.active_model.lock().unwrap() = Some(model.clone());
    vlog!(
      "Switched active model to '{}/{}'",
      model.provider,
      model.name
    );
  }

  fn resolve(&self) -> ProviderManagerResult<(Model, &dyn Provider)> {
    let model = self
      .active_model()
      .ok_or(ProviderManagerError::NoActiveModel)?;
    let provider = self
      .providers
      .iter()
      .find(|p| p.name() == model.provider)
      .map(|p| p.as_ref())
      .ok_or(ProviderManagerError::NoActiveProvider)?;
    return Ok((model, provider));
  }

  pub async fn chat(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> ProviderManagerResult<ChatResponse> {
    let (model, provider) = self.resolve()?;
    return provider.chat(messages, tools, &model.name).await;
  }

  pub async fn chat_stream(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> ProviderManagerResult<ChatStream> {
    let (model, provider) = self.resolve()?;
    return provider.chat_stream(messages, tools, &model.name).await;
  }
}

fn resolve_active_model(
  config: &Config,
  providers: &[Box<dyn Provider>],
) -> Option<Model> {
  let model_str = config.general.model.as_ref()?;
  let (provider_name, model_name) = model_str.split_once('/')?;

  let model = providers
    .iter()
    .find(|p| p.name() == provider_name)
    .and_then(|p| p.list_models().into_iter().find(|m| m.name == model_name));

  if model.is_some() {
    vlog!("Selected provider '{provider_name}' model '{model_name}'");
  } else {
    vlog!("Failed to select provider '{provider_name}' model '{model_name}'");
  }
  return model;
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
