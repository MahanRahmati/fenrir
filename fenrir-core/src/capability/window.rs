use anyhow::{Context, Result, bail};

use crate::utils::process;
use crate::utils::sleep::delay;

#[cfg(target_os = "macos")]
const LIST_APPLICATIONS: &str = r#"tell application "System Events" to get unix id of every process whose background only is false"#;

pub async fn window_ids() -> Result<Vec<String>> {
  #[cfg(target_os = "macos")]
  {
    let output = osascript(LIST_APPLICATIONS).await?;
    if !output.exit_code.is_success() {
      bail!(command_failure(&output, "osascript could not list windows"));
    }
    let ids: Vec<String> = output
      .stdout
      .split(',')
      .map(|id| id.trim().to_string())
      .filter(|id| !id.is_empty())
      .collect();
    return Ok(ids);
  }

  #[cfg(target_os = "linux")]
  {
    let output = process::run("kdotool", &["search", ".+"], None)
      .await
      .context("Failed to run kdotool")?;
    if !output.exit_code.is_success() {
      bail!(command_failure(&output, "kdotool could not list windows"));
    }
    let ids: Vec<String> = output
      .stdout
      .lines()
      .map(|line| line.trim().to_string())
      .filter(|line| !line.is_empty())
      .collect();
    return Ok(ids);
  }
}

pub async fn window_class(id: &str) -> Result<String> {
  #[cfg(target_os = "macos")]
  {
    validate_id(id)?;
    let script = format!(
      r#"tell application "System Events" to get name of first process whose unix id is {id}"#
    );
    let output = osascript(&script).await?;
    if !output.exit_code.is_success() {
      bail!(command_failure(
        &output,
        "osascript could not query the window class"
      ));
    }
    return Ok(output.stdout.trim().to_lowercase());
  }

  #[cfg(target_os = "linux")]
  {
    let output = process::run("kdotool", &["getwindowclassname", id], None)
      .await
      .context("Failed to run kdotool")?;
    if !output.exit_code.is_success() {
      bail!(command_failure(
        &output,
        "kdotool could not query the window class"
      ));
    }
    return Ok(output.stdout.trim().to_lowercase());
  }
}

pub async fn focus_window(id: &str, duration: Option<u64>) -> Result<String> {
  delay(duration).await;

  #[cfg(target_os = "macos")]
  {
    validate_id(id)?;
    let script = format!(
      r#"tell application "System Events" to set frontmost of (first process whose unix id is {id}) to true"#
    );
    let output = osascript(&script).await?;
    if !output.exit_code.is_success() {
      bail!(command_failure(
        &output,
        "osascript could not activate the window"
      ));
    }
    return Ok(id.to_string());
  }

  #[cfg(target_os = "linux")]
  {
    let output = process::run("kdotool", &["windowactivate", id], None)
      .await
      .context("Failed to run kdotool")?;
    if !output.exit_code.is_success() {
      bail!(command_failure(
        &output,
        "kdotool could not activate the window"
      ));
    }
    return Ok(id.to_string());
  }
}

#[cfg(target_os = "macos")]
async fn osascript(script: &str) -> Result<process::Output> {
  return process::run("osascript", &["-e", script], None)
    .await
    .context("Failed to run osascript");
}

#[cfg(target_os = "macos")]
fn validate_id(id: &str) -> Result<()> {
  if !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()) {
    return Ok(());
  }
  bail!("Invalid window id '{id}'");
}

fn command_failure(output: &process::Output, fallback: &str) -> String {
  let stderr = output.stderr.trim();
  if stderr.is_empty() {
    return fallback.to_string();
  }
  let mut reason = stderr.to_string();
  #[cfg(target_os = "macos")]
  {
    if stderr.contains("-1743")
      || stderr.contains("Not authorized")
      || stderr.contains("not allowed assistive access")
    {
      reason.push_str(
        " Grant Fenrir access in System Settings → Privacy & Security → \
         Automation and Accessibility.",
      );
    }
  }
  return reason;
}
