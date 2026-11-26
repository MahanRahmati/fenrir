use std::{env, path::PathBuf};

use anyhow::{Context, Result};

use crate::{files::operations, utils::xdg};

const DEFAULT_SECRETS_NAME: &str = "secrets.toml";
const OPENAI_COMPATIBLE_API_KEY: &str = "OPENAI_COMPATIBLE_API_KEY";
const OPENROUTER_API_KEY: &str = "OPENROUTER_API_KEY";
const OPENCODE_API_KEY: &str = "OPENCODE_API_KEY";
const ZAI_API_KEY: &str = "ZAI_API_KEY";

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Secrets {
  pub openai_compatible_key: Option<String>,
  pub open_router_key: Option<String>,
  pub opencode_key: Option<String>,
  pub zai_key: Option<String>,
}

impl Secrets {
  pub async fn load() -> Result<Secrets> {
    let mut secrets = match xdg::find_config_file(DEFAULT_SECRETS_NAME) {
      Some(path) => Secrets::load_from_path(path).await?,
      None => Secrets::default(),
    };
    secrets.apply_env();
    return Ok(secrets);
  }

  fn apply_env(&mut self) {
    self.apply_env_with(|name| env::var(name).ok());
  }

  fn apply_env_with(&mut self, lookup: impl Fn(&str) -> Option<String>) {
    override_key(
      &mut self.openai_compatible_key,
      lookup(OPENAI_COMPATIBLE_API_KEY),
    );
    override_key(&mut self.open_router_key, lookup(OPENROUTER_API_KEY));
    override_key(&mut self.opencode_key, lookup(OPENCODE_API_KEY));
    override_key(&mut self.zai_key, lookup(ZAI_API_KEY));
  }

  pub(crate) async fn load_from_path(secrets_path: PathBuf) -> Result<Secrets> {
    let secrets_content =
      operations::read_to_string(&secrets_path.to_string_lossy())
        .await
        .context(format!(
          "Cannot read secrets file '{}'",
          secrets_path.display()
        ))?;
    let secrets = toml::from_str(&secrets_content).context(format!(
      "Secrets file '{}' is invalid",
      secrets_path.display()
    ))?;
    return Ok(secrets);
  }
}

fn override_key(key: &mut Option<String>, value: Option<String>) {
  if let Some(value) = value.filter(|value| !value.is_empty()) {
    *key = Some(value);
  }
}
