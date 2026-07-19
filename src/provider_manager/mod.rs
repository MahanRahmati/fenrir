mod errors;
mod provider;

use crate::config::Config;
use crate::provider_manager::provider::Provider;

pub struct ProviderManager {
  providers: Vec<Provider>,
}

impl ProviderManager {
  pub fn new(config: Config) -> Self {
    return Self {
      providers: Vec::new(),
    };
  }
}
