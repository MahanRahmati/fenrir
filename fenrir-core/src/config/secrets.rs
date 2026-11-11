use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::{files::operations, utils::xdg};

const DEFAULT_SECRETS_NAME: &str = "secrets.toml";

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
    let secrets_path = match xdg::find_config_file(DEFAULT_SECRETS_NAME) {
      Some(path) => path,
      None => {
        let default_secrets = Secrets::default();
        return Ok(default_secrets);
      }
    };
    return Secrets::load_from_path(secrets_path).await;
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
