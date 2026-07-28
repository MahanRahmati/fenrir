use std::process::Stdio;

use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
  Success,
  NoMatch,
  Error(i32),
}

impl ExitCode {
  pub fn is_success(&self) -> bool {
    return matches!(self, ExitCode::Success);
  }

  pub fn is_no_match(&self) -> bool {
    return matches!(self, ExitCode::NoMatch);
  }
}

#[derive(Debug)]
pub struct Output {
  pub stdout: String,
  pub stderr: String,
  pub exit_code: ExitCode,
}

pub async fn run(program: &str, args: &[&str]) -> Result<Output, String> {
  let output = Command::new(program)
    .args(args)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()
    .await
    .map_err(|e| format!("Failed to execute '{}': {}", program, e))?;

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
