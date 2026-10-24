use anyhow::{Context, Result, bail};

use crate::utils::process;
use crate::utils::sleep::delay;

pub async fn window_ids() -> Result<Vec<String>> {
  let output = process::run("kdotool", &["search", ".+"])
    .await
    .context("Failed to run kdotool")?;
  if !output.exit_code.is_success() {
    bail!(kdotool_failure(&output, "kdotool could not list windows"));
  }
  let ids: Vec<String> = output
    .stdout
    .lines()
    .map(|line| line.trim().to_string())
    .filter(|line| !line.is_empty())
    .collect();
  return Ok(ids);
}

pub async fn window_class(id: &str) -> Result<String> {
  let output = process::run("kdotool", &["getwindowclassname", id])
    .await
    .context("Failed to run kdotool")?;
  if !output.exit_code.is_success() {
    bail!(kdotool_failure(
      &output,
      "kdotool could not query the window class"
    ));
  }
  return Ok(output.stdout.trim().to_lowercase());
}

pub async fn focus_window(id: &str, duration: Option<u64>) -> Result<String> {
  delay(duration).await;
  let output = process::run("kdotool", &["windowactivate", id])
    .await
    .context("Failed to run kdotool")?;
  if !output.exit_code.is_success() {
    bail!(kdotool_failure(
      &output,
      "kdotool could not activate the window"
    ));
  }
  return Ok(id.to_string());
}

fn kdotool_failure(output: &process::Output, fallback: &str) -> String {
  let stderr = output.stderr.trim();
  if stderr.is_empty() {
    return fallback.to_string();
  }
  return stderr.to_string();
}
