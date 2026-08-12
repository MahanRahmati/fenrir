use std::time::Duration;

use tokio::time;

use crate::capability::errors::*;
use crate::capability::keyboard;
use crate::utils::process;

pub async fn active_window() -> CapabilityErrorResult<String> {
  let output = process::run(
    "kdotool",
    &["getactivewindow", "getwindowname", "getwindowclassname"],
  )
  .await
  .map_err(|e| CapabilityError::Window {
    reason: format!("Failed to run kdotool: {e}"),
  })?;
  if !output.exit_code.is_success() {
    return Err(CapabilityError::Window {
      reason: kdotool_failure(
        &output,
        "kdotool could not query the active window",
      ),
    });
  }

  let mut lines = output.stdout.trim().lines();
  let title = lines.next().unwrap_or_default().trim();
  let class = lines.next().unwrap_or_default().trim();
  return Ok(format!("Title: {title}\nClass: {class}"));
}

pub async fn window_ids() -> CapabilityErrorResult<Vec<String>> {
  let output =
    process::run("kdotool", &["search", ".+"])
      .await
      .map_err(|e| CapabilityError::Window {
        reason: format!("Failed to run kdotool: {e}"),
      })?;
  if !output.exit_code.is_success() {
    return Err(CapabilityError::Window {
      reason: kdotool_failure(&output, "kdotool could not list windows"),
    });
  }
  let ids: Vec<String> = output
    .stdout
    .lines()
    .map(|line| line.trim().to_string())
    .filter(|line| !line.is_empty())
    .collect();
  return Ok(ids);
}

pub async fn activate_window(
  before: &[String],
  delay: Option<u64>,
) -> CapabilityErrorResult<String> {
  if let Some(delay) = delay {
    time::sleep(Duration::from_secs(delay)).await;
  }

  let after = window_ids().await?;
  let new_windows: Vec<&String> =
    after.iter().filter(|id| !before.contains(id)).collect();

  let id = if let Some(id) = new_windows.last() {
    id.to_string()
  } else {
    keyboard::press_key_on_keyboard("alt+tab").await?;
    time::sleep(Duration::from_millis(300)).await;
    return Ok("Switched the window focus with alt+tab".to_string());
  };

  let output = process::run("kdotool", &["windowactivate", &id])
    .await
    .map_err(|e| CapabilityError::Window {
      reason: format!("Failed to run kdotool: {e}"),
    })?;
  if !output.exit_code.is_success() {
    return Err(CapabilityError::Window {
      reason: kdotool_failure(&output, "kdotool could not activate the window"),
    });
  }

  return Ok(id);
}

fn kdotool_failure(output: &process::Output, fallback: &str) -> String {
  let stderr = output.stderr.trim();
  if stderr.is_empty() {
    return fallback.to_string();
  }
  return stderr.to_string();
}
