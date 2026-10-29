use std::path::PathBuf;

use anyhow::{Context, Result};

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
  pub hooks: HooksConfig,
  pub compaction: CompactionConfig,
  pub subagent: SubagentConfig,
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
  pub read_pdf: bool,
  pub project_context: bool,
  pub screenshot: bool,
  pub notify: bool,
  pub control_keyboard: bool,
  pub music: bool,
  pub browser: bool,
  pub ddg: bool,
  pub goal_check: bool,
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
      read_pdf: true,
      project_context: true,
      screenshot: true,
      notify: true,
      control_keyboard: true,
      music: true,
      browser: true,
      ddg: true,
      goal_check: true,
    };
  }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HooksConfig {
  pub hook_timeout_seconds: u64,
  pub rust: bool,
  pub go: bool,
}

impl Default for HooksConfig {
  fn default() -> Self {
    return Self {
      hook_timeout_seconds: 60,
      rust: true,
      go: true,
    };
  }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct CompactionConfig {
  pub threshold: Option<u64>,
  pub tool_result_max_chars: usize,
}

impl Default for CompactionConfig {
  fn default() -> Self {
    return Self {
      threshold: None,
      tool_result_max_chars: 2000,
    };
  }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct SubagentConfig {
  pub enabled: bool,
  pub max_iterations: usize,
}

impl Default for SubagentConfig {
  fn default() -> Self {
    return Self {
      enabled: true,
      max_iterations: 100,
    };
  }
}

impl Config {
  pub async fn load() -> Result<Config> {
    let config_path = match xdg::find_config_file(DEFAULT_CONFIG_NAME) {
      Some(path) => path,
      None => {
        let default_config = Config::default();
        return Ok(default_config);
      }
    };
    return Config::load_from_path(config_path).await;
  }

  pub async fn reset_to_defaults() -> Result<()> {
    let default_config = Config::default();
    let config_path = xdg::place_config_file(DEFAULT_CONFIG_NAME)?;
    return Config::save_to_path(default_config, config_path).await;
  }

  pub(crate) async fn load_from_path(config_path: PathBuf) -> Result<Config> {
    let config_content =
      operations::read_to_string(&config_path.to_string_lossy())
        .await
        .context(format!(
          "Cannot read configuration file '{}'",
          config_path.display()
        ))?;
    let config = toml::from_str(&config_content).context(format!(
      "Configuration file '{}' is invalid",
      config_path.display()
    ))?;
    return Ok(config);
  }

  pub(crate) async fn save_to_path(
    config: Config,
    config_path: PathBuf,
  ) -> Result<()> {
    let config_content = toml::to_string_pretty(&config)
      .context("Failed to serialize configuration")?;
    operations::write_file(&config_path.to_string_lossy(), &config_content)
      .await
      .context(format!(
        "Cannot save configuration file '{}'",
        config_path.display()
      ))?;
    return Ok(());
  }
}
