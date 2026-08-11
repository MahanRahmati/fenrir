pub mod errors;

use std::path::PathBuf;

use crate::config::errors::{ConfigError, ConfigResult};
use crate::files::operations;
use crate::utils::xdg;

pub const DEFAULT_DIRECTORY: &str = "fenrir";
const DEFAULT_CONFIG_NAME: &str = "config.toml";

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Config {
  pub general: GeneralConfig,
  pub providers: ProvidersConfig,
  pub permissions: PermissionsConfig,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct GeneralConfig {
  pub model: Option<String>,
  pub stream: bool,
  pub show_reasoning: bool,
  pub show_tools: bool,
  pub tool_timeout_seconds: u64,
  pub restrict_write_tools_to_cwd: bool,
}

impl Default for GeneralConfig {
  fn default() -> Self {
    return Self {
      model: None,
      stream: true,
      show_reasoning: true,
      show_tools: true,
      tool_timeout_seconds: 60,
      restrict_write_tools_to_cwd: true,
    };
  }
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ProvidersConfig {
  pub openai_compatible: Option<OpenAICompatibleConfig>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OpenAICompatibleConfig {
  pub url: Option<String>,
  pub name: String,
  pub api_key: Option<String>,
  pub models: Vec<ProviderModel>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ProviderModel {
  pub name: String,
  pub alias: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct PermissionsConfig {
  pub datetime: bool,
  pub read_file: bool,
  pub write_file: bool,
  pub edit_file: bool,
  pub remove_file: bool,
  pub find_file: bool,
  pub find_text: bool,
  pub list_directories: bool,
  pub project_context: bool,
  pub rust: bool,
  pub go: bool,
  pub screenshot: bool,
  pub notify: bool,
  pub control_keyboard: bool,
  pub music: bool,
}

impl Default for PermissionsConfig {
  fn default() -> Self {
    return Self {
      datetime: true,
      read_file: true,
      write_file: true,
      edit_file: true,
      remove_file: true,
      find_file: true,
      find_text: true,
      list_directories: true,
      project_context: true,
      rust: true,
      go: true,
      screenshot: true,
      notify: true,
      control_keyboard: true,
      music: true,
    };
  }
}

impl Config {
  pub async fn load() -> ConfigResult<Config> {
    let config_path = match xdg::find_config_file(DEFAULT_CONFIG_NAME) {
      Some(path) => path,
      None => {
        let default_config = Config::default();
        return Ok(default_config);
      }
    };
    return Config::load_from_path(config_path).await;
  }

  pub async fn reset_to_defaults(yes: bool) -> ConfigResult<()> {
    if !yes {
      let should_reset = crate::input::confirm_with(
        "Are you sure you want to reset the configuration to default values? (y/n) ",
      );
      if !should_reset {
        println!("Reset cancelled.");
        return Ok(());
      }
    }
    let default_config = Config::default();
    let config_path = xdg::place_config_file(DEFAULT_CONFIG_NAME)
      .map_err(ConfigError::FileRead)?;
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
    operations::write_file(&config_path.to_string_lossy(), &config_content)
      .await
      .map_err(|e| ConfigError::FileWrite(e.to_string()))?;
    return Ok(());
  }
}
