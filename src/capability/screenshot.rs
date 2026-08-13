use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::Local;
use tokio::time;

use crate::capability::errors::*;
use crate::files::operations::{
  copy_file, create_directory_all, get_file_size, get_modified_time, read_dir,
  remove_file,
};
use crate::utils::image::optimize_image;
use crate::utils::process;
use crate::utils::sleep::poll;
use crate::utils::xdg;

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

pub async fn screenshot(
  path: Option<String>,
  delay: Option<u64>,
) -> CapabilityErrorResult<String> {
  if let Some(delay) = delay {
    time::sleep(Duration::from_secs(delay)).await;
  }

  let dirs = picture_dirs();
  let snapshot = existing_pngs(&dirs).await;
  let call_start = SystemTime::now();

  let output =
    process::run("dbus-send", SCREENSHOT_ARGS)
      .await
      .map_err(|e| CapabilityError::Screenshot {
        reason: format!("Failed to invoke dbus-send: {e}"),
      })?;

  if !output.exit_code.is_success() {
    let stderr = output.stderr.trim();
    let details = if stderr.is_empty() {
      "the desktop portal could not take a screenshot".to_string()
    } else {
      stderr.to_string()
    };
    return Err(CapabilityError::Screenshot { reason: details });
  }

  let source = match wait_for_new_png(&dirs, &snapshot, call_start).await {
    Some(path) => path,
    None => {
      return Err(CapabilityError::Screenshot {
        reason: format!(
          "Screenshot was taken but no new image file appeared in {dirs:?}"
        ),
      });
    }
  };

  let destination = match path {
    Some(path) => path,
    None => default_destination().map_err(|e| CapabilityError::Screenshot {
      reason: format!("Cannot determine save location: {e}"),
    })?,
  };
  let destination = resolve_destination(destination).await.map_err(|e| {
    CapabilityError::Screenshot {
      reason: format!("Cannot determine save location: {e}"),
    }
  })?;

  if let Some(parent) = Path::new(&destination).parent()
    && let Some(parent_str) = parent.as_os_str().to_str()
  {
    create_directory_all(parent_str).await.map_err(|e| {
      CapabilityError::Screenshot {
        reason: format!(
          "Cannot create parent directories for '{destination}': {e}"
        ),
      }
    })?;
  }

  copy_file(&source.to_string_lossy(), &destination)
    .await
    .map_err(|e| CapabilityError::Screenshot {
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
    return Err(CapabilityError::Screenshot {
      reason: format!(
        "Copy of '{}' to '{destination}' is incomplete ({dest_size} of {source_size} bytes). The original was left in place.",
        source.display()
      ),
    });
  }

  let destination = optimize_image(&destination).await;

  remove_file(&source.to_string_lossy()).await.map_err(|e| {
      CapabilityError::Screenshot {
        reason: format!(
          "Screenshot saved to '{destination}' but the original could not be removed: {e}"
        ),
      }
    })?;

  return Ok(destination);
}

async fn resolve_destination(destination: String) -> Result<String, String> {
  let path = Path::new(&destination);
  if path.is_dir()
    || destination.ends_with(std::path::MAIN_SEPARATOR)
    || path.extension().is_none()
  {
    let dir = destination.trim_end_matches(std::path::MAIN_SEPARATOR);
    create_directory_all(dir)
      .await
      .map_err(|e| format!("Cannot create directory '{dir}': {e}"))?;
    return Ok(
      Path::new(dir)
        .join(timestamped_name("screenshot"))
        .to_string_lossy()
        .to_string(),
    );
  }
  return Ok(destination);
}

fn timestamped_name(prefix: &str) -> String {
  let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
  return format!("{prefix}_{timestamp}.png");
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
  let data_home = xdg::data_home()?;
  let dir = data_home.join("screenshots");
  return Ok(
    dir
      .join(timestamped_name("screenshot"))
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
  return poll(POLL_TIMEOUT, POLL_INTERVAL, || {
    newest_png(dirs, snapshot, start)
  })
  .await;
}

async fn newest_png(
  dirs: &[PathBuf],
  snapshot: &[PathBuf],
  start: SystemTime,
) -> Option<PathBuf> {
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
  return None;
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
