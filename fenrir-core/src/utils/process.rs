use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
  Success,
  NoMatch,
  Error(i32),
}

impl ExitCode {
  pub fn is_success(self) -> bool {
    return matches!(self, ExitCode::Success);
  }

  pub fn is_no_match(self) -> bool {
    return matches!(self, ExitCode::NoMatch);
  }
}

#[derive(Debug)]
pub struct Output {
  pub stdout: String,
  pub stderr: String,
  pub exit_code: ExitCode,
}

pub async fn run(
  program: &str,
  args: &[&str],
  cwd: Option<&Path>,
) -> Result<Output> {
  let mut command = Command::new(program);
  command.args(args);
  if let Some(cwd) = cwd {
    command.current_dir(cwd);
  }
  let output = command
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true)
    .output()
    .await
    .context(format!("Failed to execute '{program}'"))?;

  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let stderr = String::from_utf8_lossy(&output.stderr).to_string();
  let exit_code = match output.status.code() {
    Some(0) => ExitCode::Success,
    Some(1) => ExitCode::NoMatch,
    Some(code) => ExitCode::Error(code),
    None => ExitCode::Error(-1),
  };

  return Ok(Output {
    stdout,
    stderr,
    exit_code,
  });
}

#[cfg(target_os = "macos")]
pub async fn osascript(script: &str) -> Result<Output> {
  return run("osascript", &["-e", script], None)
    .await
    .context("Failed to run osascript");
}

#[cfg(target_os = "macos")]
pub fn escape_applescript(text: &str) -> String {
  let mut escaped = String::with_capacity(text.len());
  for c in text.chars() {
    match c {
      '\\' => escaped.push_str("\\\\"),
      '"' => escaped.push_str("\\\""),
      _ => escaped.push(c),
    }
  }
  return escaped;
}

pub fn failure(output: &Output, fallback: &str) -> String {
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
