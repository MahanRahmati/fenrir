use anyhow::{Context, Result, bail};

use crate::utils::process;

pub async fn send_notification(summary: &str, body: &str) -> Result<String> {
  let summary_arg = format!("string:{summary}");
  let body_arg = format!("string:{body}");

  let args: Vec<&str> = vec![
    "--session",
    "--dest=org.freedesktop.Notifications",
    "--type=method_call",
    "--print-reply",
    "/org/freedesktop/Notifications",
    "org.freedesktop.Notifications.Notify",
    "string:Fenrir",
    "uint32:0",
    "string:",
    &summary_arg,
    &body_arg,
    "array:string:",
    "dict:string:variant:",
    "int32:-1",
  ];

  let output = process::run("dbus-send", &args)
    .await
    .context("Failed to invoke dbus-send")?;

  if !output.exit_code.is_success() {
    let stderr = output.stderr.trim();
    let details = if stderr.is_empty() {
      "the notification could not be sent".to_string()
    } else {
      stderr.to_string()
    };
    bail!("{details}");
  }

  return Ok("Notification sent".to_string());
}
