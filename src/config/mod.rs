pub mod errors;

use std::path::PathBuf;

use xdg::BaseDirectories;

use crate::config::errors::{ConfigError, ConfigResult};
use crate::files::operations;

const DEFAULT_DIRECTORY: &str = "fenrir";
const DEFAULT_CONFIG_NAME: &str = "config.toml";
const DEFAULT_MODEL_URL: &str = "http://127.0.0.1:12434";
const DEFAULT_MODEL_NAME: &str = "qwen-3.6-35B-A3B";

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Config {
  pub model: ModelConfig,
  pub general: GeneralConfig,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ModelConfig {
  pub url: Option<String>,
  pub name: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct GeneralConfig {}

impl Config {
  pub async fn load() -> ConfigResult<Config> {
    let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
    let config_path = match xdg_dirs.find_config_file(DEFAULT_CONFIG_NAME) {
      Some(path) => path,
      None => {
        let default_config = Config::default();
        return Ok(default_config);
      }
    };
    return Config::load_from_path(config_path).await;
  }

  pub fn get_model_url(&self) -> String {
    return self
      .model
      .url
      .clone()
      .unwrap_or(String::from(DEFAULT_MODEL_URL));
  }

  pub fn get_model_name(&self) -> String {
    return self
      .model
      .name
      .clone()
      .unwrap_or(String::from(DEFAULT_MODEL_NAME));
  }

  pub async fn reset_to_defaults() -> ConfigResult<()> {
    let default_config = Config::default();
    let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
    let config_path = xdg_dirs
      .place_config_file(DEFAULT_CONFIG_NAME)
      .map_err(|e| ConfigError::FileRead(e.to_string()))?;
    return Config::save_to_path(default_config, config_path).await;
  }

  pub(crate) async fn load_from_path(
    config_path: PathBuf,
  ) -> ConfigResult<Config> {
    let config_content =
      operations::read_to_string(&config_path.to_string_lossy())
        .await
        .map_err(|e| ConfigError::FileRead(e.to_string()))?;
    let config = toml::from_str(&config_content)
      .map_err(|e| ConfigError::Parse(e.to_string()))?;
    return Ok(config);
  }

  pub(crate) async fn save_to_path(
    config: Config,
    config_path: PathBuf,
  ) -> ConfigResult<()> {
    let config_content = toml::to_string_pretty(&config)
      .map_err(|e| ConfigError::Parse(e.to_string()))?;
    tokio::fs::write(&config_path, config_content)
      .await
      .map_err(|e| ConfigError::FileRead(e.to_string()))?;
    return Ok(());
  }
}

impl Default for Config {
  fn default() -> Self {
    return Config {
      model: ModelConfig {
        url: Some(String::from(DEFAULT_MODEL_URL)),
        name: Some(String::from(DEFAULT_MODEL_NAME)),
      },
      general: GeneralConfig {},
    };
  }
}
