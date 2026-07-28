pub mod errors;

use std::path::PathBuf;

use xdg::BaseDirectories;

use crate::config::errors::{ConfigError, ConfigResult};
use crate::files::operations;

pub const DEFAULT_DIRECTORY: &str = "fenrir";
const DEFAULT_CONFIG_NAME: &str = "config.toml";
const DEFAULT_STREAM: bool = true;
const DEFAULT_TOOL_TIMEOUT_SECONDS: u64 = 5;
const DEFAULT_DATETIME_PERMISSION: bool = true;
const DEFAULT_READ_FILE_PERMISSION: bool = true;
const DEFAULT_WRITE_FILE_PERMISSION: bool = true;
const DEFAULT_EDIT_FILE_PERMISSION: bool = true;
const DEFAULT_REMOVE_FILE_PERMISSION: bool = true;
const DEFAULT_FIND_FILE_PERMISSION: bool = true;
const DEFAULT_FIND_TEXT_PERMISSION: bool = true;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Config {
  #[serde(default)]
  pub general: GeneralConfig,
  #[serde(default)]
  pub providers: ProvidersConfig,
  #[serde(default)]
  pub permissions: PermissionsConfig,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct GeneralConfig {
  pub model: Option<String>,
  #[serde(default = "default_stream")]
  pub stream: bool,
  #[serde(default = "default_tool_timeout")]
  pub tool_timeout_seconds: u64,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
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

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct PermissionsConfig {
  #[serde(default = "default_datetime")]
  pub datetime: bool,
  #[serde(default = "default_read_file")]
  pub read_file: bool,
  #[serde(default = "default_write_file")]
  pub write_file: bool,
  #[serde(default = "default_edit_file")]
  pub edit_file: bool,
  #[serde(default = "default_remove_file")]
  pub remove_file: bool,
  #[serde(default = "default_find_file")]
  pub find_file: bool,
  #[serde(default = "default_find_text")]
  pub find_text: bool,
}

fn default_stream() -> bool {
  DEFAULT_STREAM
}

fn default_tool_timeout() -> u64 {
  DEFAULT_TOOL_TIMEOUT_SECONDS
}

fn default_datetime() -> bool {
  DEFAULT_DATETIME_PERMISSION
}
fn default_read_file() -> bool {
  DEFAULT_READ_FILE_PERMISSION
}
fn default_write_file() -> bool {
  DEFAULT_WRITE_FILE_PERMISSION
}
fn default_edit_file() -> bool {
  DEFAULT_EDIT_FILE_PERMISSION
}
fn default_remove_file() -> bool {
  DEFAULT_REMOVE_FILE_PERMISSION
}
fn default_find_file() -> bool {
  DEFAULT_FIND_FILE_PERMISSION
}
fn default_find_text() -> bool {
  DEFAULT_FIND_TEXT_PERMISSION
}

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
    operations::write_file(&config_path.to_string_lossy(), &config_content)
      .await
      .map_err(|e| ConfigError::FileWrite(e.to_string()))?;
    return Ok(());
  }
}

impl Default for Config {
  fn default() -> Self {
    return Config {
      general: GeneralConfig {
        model: None,
        stream: DEFAULT_STREAM,
        tool_timeout_seconds: DEFAULT_TOOL_TIMEOUT_SECONDS,
      },
      providers: ProvidersConfig {
        openai_compatible: None,
      },
      permissions: PermissionsConfig {
        datetime: DEFAULT_DATETIME_PERMISSION,
        read_file: DEFAULT_READ_FILE_PERMISSION,
        write_file: DEFAULT_WRITE_FILE_PERMISSION,
        edit_file: DEFAULT_EDIT_FILE_PERMISSION,
        remove_file: DEFAULT_REMOVE_FILE_PERMISSION,
        find_file: DEFAULT_FIND_FILE_PERMISSION,
        find_text: DEFAULT_FIND_TEXT_PERMISSION,
      },
    };
  }
}
