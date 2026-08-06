use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use chrono::Local;
use tokio::time;
use xdg::BaseDirectories;

use crate::config::DEFAULT_DIRECTORY;
use crate::files::operations::{
  copy_file, create_directory_all, get_file_size, get_modified_time, read_dir,
  remove_file,
};
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::get_optional_param;
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;

pub struct ScreenshotTool;

const SCREENSHOT_ARGS: &[&str] = &[
  "--session",
  "--dest=org.freedesktop.portal.Desktop",
  "--type=method_call",
  "--print-reply",
  "/org/freedesktop/portal/desktop",
  "org.freedesktop.portal.Screenshot.Screenshot",
  "string:",
  "dict:string:variant:interactive,boolean:false",
];

const POLL_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const STABILITY_INTERVAL: Duration = Duration::from_millis(300);

#[async_trait::async_trait]
impl Tool for ScreenshotTool {
  fn name(&self) -> &str {
    return "screenshot";
  }

  fn description(&self) -> &str {
    return "Take a full-screen screenshot and save it to a file. Returns the path of the saved screenshot.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "path": {"type": "string", "description": "Where to save the screenshot. Defaults to a file in the fenrir data directory."},
      },
      "required": [],
    });
  }

  fn is_read_only(&self) -> bool {
    return false;
  }

  async fn execute(&self, params: ToolParams) -> ToolManagerResult<String> {
    let dirs = picture_dirs();
    let snapshot = existing_pngs(&dirs).await;
    let call_start = SystemTime::now();

    let output =
      process::run("dbus-send", SCREENSHOT_ARGS)
        .await
        .map_err(|e| ToolManagerError::ExecutionFailed {
          name: "screenshot".to_string(),
          reason: format!("Failed to invoke dbus-send: {e}"),
        })?;

    if !output.exit_code.is_success() {
      let stderr = output.stderr.trim();
      let details = if stderr.is_empty() {
        "the desktop portal could not take a screenshot".to_string()
      } else {
        stderr.to_string()
      };
      return Err(ToolManagerError::ExecutionFailed {
        name: "screenshot".to_string(),
        reason: details,
      });
    }

    let source = match wait_for_new_png(&dirs, &snapshot, call_start).await {
      Some(path) => path,
      None => {
        return Err(ToolManagerError::ExecutionFailed {
          name: "screenshot".to_string(),
          reason: format!(
            "Screenshot was taken but no new image file appeared in {dirs:?}"
          ),
        });
      }
    };

    let destination =
      match get_optional_param::<String>(&params, "screenshot", "path") {
        Some(path) => path,
        None => default_destination().map_err(|e| {
          ToolManagerError::ExecutionFailed {
            name: "screenshot".to_string(),
            reason: format!("Cannot determine save location: {e}"),
          }
        })?,
      };

    if let Some(parent) = Path::new(&destination).parent()
      && let Some(parent_str) = parent.as_os_str().to_str()
    {
      create_directory_all(parent_str).await.map_err(|e| {
        ToolManagerError::ExecutionFailed {
          name: "screenshot".to_string(),
          reason: format!(
            "Cannot create parent directories for '{destination}': {e}"
          ),
        }
      })?;
    }

    copy_file(&source.to_string_lossy(), &destination)
      .await
      .map_err(|e| ToolManagerError::ExecutionFailed {
        name: "screenshot".to_string(),
        reason: format!(
          "Cannot copy '{}' to '{destination}': {e}",
          source.display()
        ),
      })?;

    let source_size = get_file_size(&source.to_string_lossy()).await;
    let dest_size = get_file_size(&destination).await;
    if let (Ok(source_size), Ok(dest_size)) = (source_size, dest_size)
      && source_size != dest_size
    {
      return Err(ToolManagerError::ExecutionFailed {
        name: "screenshot".to_string(),
        reason: format!(
          "Copy of '{}' to '{destination}' is incomplete ({dest_size} of {source_size} bytes). The original was left in place.",
          source.display()
        ),
      });
    }

    remove_file(&source.to_string_lossy()).await.map_err(|e| {
      ToolManagerError::ExecutionFailed {
        name: "screenshot".to_string(),
        reason: format!(
          "Screenshot saved to '{destination}' but the original could not be removed: {e}"
        ),
      }
    })?;

    return Ok(format!("Screenshot saved to '{destination}'"));
  }
}

fn picture_dirs() -> Vec<PathBuf> {
  let mut dirs = Vec::new();
  if let Ok(dir) = std::env::var("XDG_PICTURES_DIR") {
    dirs.push(PathBuf::from(dir));
  }
  if let Ok(home) = std::env::var("HOME") {
    dirs.push(Path::new(&home).join("Pictures"));
  }
  return dirs;
}

fn default_destination() -> Result<String, String> {
  let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
  let data_home = xdg_dirs
    .get_data_home()
    .ok_or_else(|| "cannot determine the data home directory".to_string())?;
  let dir = data_home.join("screenshots");
  let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
  return Ok(
    dir
      .join(format!("screenshot_{timestamp}.png"))
      .to_string_lossy()
      .to_string(),
  );
}

async fn existing_pngs(dirs: &[PathBuf]) -> Vec<PathBuf> {
  let mut files = Vec::new();
  for dir in dirs {
    if let Ok(entries) = read_dir(&dir.to_string_lossy()).await {
      for (name, _) in entries {
        if name.ends_with(".png") {
          files.push(dir.join(name));
        }
      }
    }
  }
  return files;
}

async fn wait_for_new_png(
  dirs: &[PathBuf],
  snapshot: &[PathBuf],
  start: SystemTime,
) -> Option<PathBuf> {
  let deadline = Instant::now() + POLL_TIMEOUT;
  loop {
    for path in existing_pngs(dirs).await {
      if snapshot.contains(&path) {
        continue;
      }
      let modified = get_modified_time(&path.to_string_lossy())
        .await
        .unwrap_or(start);
      if modified >= start && is_stable(&path).await {
        return Some(path);
      }
    }
    if Instant::now() >= deadline {
      return None;
    }
    time::sleep(POLL_INTERVAL).await;
  }
}

async fn is_stable(path: &Path) -> bool {
  let first = get_file_size(&path.to_string_lossy()).await.ok();
  time::sleep(STABILITY_INTERVAL).await;
  let second = get_file_size(&path.to_string_lossy()).await.ok();
  return match (first, second) {
    (Some(first), Some(second)) => first == second && first > 0,
    _ => false,
  };
}
