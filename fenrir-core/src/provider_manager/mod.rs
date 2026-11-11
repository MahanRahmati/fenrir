mod provider;
mod providers;

use anyhow::{Result, bail};

use crate::config::Config;
use crate::config::secrets::Secrets;
use crate::network::NetworkTimeouts;
pub use crate::provider_manager::provider::ChatUsage;
pub use crate::provider_manager::provider::Model;
pub use crate::provider_manager::provider::ToolCall;
use crate::provider_manager::provider::{ChatResponse, ChatStream, Provider};
use crate::provider_manager::providers::openai_compatible::OpenAICompatibleProvider;
use crate::provider_manager::providers::opencode::OpencodeProvider;
use crate::provider_manager::providers::openrouter::OpenRouterProvider;
use crate::provider_manager::providers::zai::ZaiProvider;
use crate::session_manager::Message;
use crate::tool_manager::tool::ToolSpec;
use crate::vlog;

pub struct ProviderManager {
  providers: Vec<Box<dyn Provider>>,
  default_model: Option<Model>,
}

impl ProviderManager {
  pub fn new(config: Config, secrets: Secrets) -> Self {
    let mut providers: Vec<Box<dyn Provider>> = Vec::new();
    let timeouts = NetworkTimeouts::from_config(&config.network);
    if let Some(provider) = setup_openai_compatible(
      config.clone(),
      secrets.openai_compatible_key.clone(),
      timeouts,
    ) {
      providers.push(Box::new(provider));
    }
    if let Some(provider) = setup_openrouter(
      config.clone(),
      secrets.open_router_key.clone(),
      timeouts,
    ) {
      providers.push(Box::new(provider));
    }
    if let Some(provider) =
      setup_opencode(config.clone(), secrets.opencode_key.clone(), timeouts)
    {
      providers.push(Box::new(provider));
    }
    if let Some(provider) =
      setup_zai(config.clone(), secrets.zai_key.clone(), timeouts)
    {
      providers.push(Box::new(provider));
    }
    let default_model = resolve_default_model(&config, &providers);
    return Self {
      providers,
      default_model,
    };
  }

  pub fn list_models(&self) -> Vec<Model> {
    let mut models = Vec::new();
    for provider in self.providers.iter() {
      models.extend(provider.list_models());
    }
    return models;
  }

  pub fn default_model(&self) -> Option<Model> {
    return self.default_model.clone();
  }

  pub async fn context_window(&self, model: &Model) -> Option<u64> {
    let provider = match self.get_provider(&model.provider) {
      Ok(provider) => provider,
      Err(e) => {
        vlog!("Failed to resolve provider for context window: {e}");
        return None;
      }
    };
    return match provider.context_window(&model.name).await {
      Ok(context_window) => context_window,
      Err(e) => {
        vlog!("Failed to fetch context window: {e}");
        None
      }
    };
  }

  fn get_provider(&self, name: &str) -> Result<&dyn Provider> {
    let Some(provider) = self
      .providers
      .iter()
      .find(|p| p.name() == name)
      .map(|p| p.as_ref())
    else {
      bail!("No provider configured for '{name}'");
    };
    return Ok(provider);
  }

  pub async fn chat(
    &self,
    model: &Model,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> Result<ChatResponse> {
    return self
      .get_provider(&model.provider)?
      .chat(messages, tools, &model.name)
      .await;
  }

  pub async fn chat_stream(
    &self,
    model: &Model,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> Result<ChatStream> {
    return self
      .get_provider(&model.provider)?
      .chat_stream(messages, tools, &model.name)
      .await;
  }
}

fn resolve_default_model(
  config: &Config,
  providers: &[Box<dyn Provider>],
) -> Option<Model> {
  if let Some(model_str) = config.general.model.as_ref() {
    if let Some((provider_name, model_name)) = model_str.split_once('/') {
      if let Some(model) = providers
        .iter()
        .find(|p| p.name() == provider_name)
        .and_then(|p| {
          p.list_models().into_iter().find(|m| m.name == model_name)
        })
      {
        vlog!("Selected provider '{provider_name}' model '{model_name}'");
        return Some(model);
      }
      vlog!("Failed to select provider '{provider_name}' model '{model_name}'");
    } else {
      vlog!(
        "Invalid configured model '{model_str}': expected '<provider>/<model>'"
      );
    }
  }

  let model = providers
    .iter()
    .find_map(|p| p.list_models().into_iter().next());
  if let Some(model) = &model {
    vlog!(
      "Using default provider '{}' model '{}'",
      model.provider,
      model.name
    );
  }
  return model;
}

fn setup_openai_compatible(
  config: Config,
  api_key: Option<String>,
  timeouts: NetworkTimeouts,
) -> Option<OpenAICompatibleProvider> {
  let config = config.providers.openai_compatible?;
  let url = config.url.as_ref()?;
  if config.models.is_empty() {
    return None;
  }
  return Some(OpenAICompatibleProvider::new(
    url.clone(),
    config.name.clone(),
    api_key,
    config.models.clone(),
    timeouts,
  ));
}

fn setup_openrouter(
  config: Config,
  api_key: Option<String>,
  timeouts: NetworkTimeouts,
) -> Option<OpenRouterProvider> {
  let config = config.providers.open_router?;
  if config.models.is_empty() {
    return None;
  }
  return Some(OpenRouterProvider::new(
    api_key,
    config.models.clone(),
    timeouts,
  ));
}

fn setup_opencode(
  config: Config,
  api_key: Option<String>,
  timeouts: NetworkTimeouts,
) -> Option<OpencodeProvider> {
  let config = config.providers.opencode?;
  if config.models.is_empty() {
    return None;
  }
  return Some(OpencodeProvider::new(
    api_key,
    config.models.clone(),
    timeouts,
  ));
}

fn setup_zai(
  config: Config,
  api_key: Option<String>,
  timeouts: NetworkTimeouts,
) -> Option<ZaiProvider> {
  let config = config.providers.zai?;
  if config.models.is_empty() {
    return None;
  }
  return Some(ZaiProvider::new(api_key, config.models.clone(), timeouts));
}
