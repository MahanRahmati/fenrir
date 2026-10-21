use std::time::Duration;

use tokio::time;

use crate::utils::process;
use crate::vlog;

pub async fn run_command(
  hook: &str,
  program: &str,
  args: &[&str],
  timeout: Duration,
) -> String {
  let command = if args.is_empty() {
    program.to_string()
  } else {
    format!("{program} {}", args.join(" "))
  };

  vlog!("Hook '{}' running '{}'", hook, command);

  let output = match time::timeout(timeout, process::run(program, args)).await {
    Ok(Ok(output)) => output,
    Ok(Err(reason)) => {
      return format!("\n\nHook '{hook}' failed to run '{command}': {reason}");
    }
    Err(_) => {
      return format!("\n\nHook '{hook}' timed out while running '{command}'");
    }
  };

  if output.exit_code.is_success() {
    return String::new();
  }

  let stderr = output.stderr.trim();
  let stdout = output.stdout.trim();
  let details = if !stderr.is_empty() {
    stderr
  } else if !stdout.is_empty() {
    stdout
  } else {
    "command exited with a non-zero status."
  };

  return format!("\n\n[{hook} hook] {command} failed:\n{details}");
}

pub fn tool_path(arguments: &str) -> Option<String> {
  let value: serde_json::Value = serde_json::from_str(arguments).ok()?;
  return value
    .get("path")
    .and_then(|path| path.as_str())
    .map(str::to_string);
}
