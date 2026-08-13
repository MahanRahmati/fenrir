use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::capability::errors::*;
use crate::capability::keyboard;
use crate::capability::window;
use crate::files::operations::{
  create_directory_all, get_file_size, get_modified_time, read_dir, rename_file,
};
use crate::utils::process;
use crate::utils::sleep::{delay, milli_delay, poll};
use crate::utils::xdg;

const DEFAULT_FOCUS_DELAY: u64 = 5;
const PRINT_DIALOG_DELAY_SECS: u64 = 10;
const DIALOG_STEP_DELAY_SECS: u64 = 2;
const KEY_PACING_MS: u64 = 400;
const SAVE_POLL_TIMEOUT_SECS: u64 = 30;
const SAVE_POLL_INTERVAL_MS: u64 = 300;

pub async fn open_url(url: &str) -> CapabilityErrorResult<String> {
  let output = process::run("xdg-open", &[url]).await.map_err(|e| {
    CapabilityError::Browser {
      reason: format!("Failed to invoke xdg-open: {e}"),
    }
  })?;

  if !output.exit_code.is_success() {
    let stderr = output.stderr.trim();
    let details = if stderr.is_empty() {
      "the URL could not be opened".to_string()
    } else {
      stderr.to_string()
    };
    return Err(CapabilityError::Browser { reason: details });
  }

  delay(Some(DEFAULT_FOCUS_DELAY)).await;
  focus_browser().await?;

  return Ok(format!("Opened {url} in the default browser"));
}

pub async fn save_as_pdf() -> CapabilityErrorResult<String> {
  let pdf_dir = pdf_dir().await?;
  let destination = pdf_destination(&pdf_dir);
  let start = SystemTime::now();

  focus_browser().await?;
  open_print_dialog().await?;
  open_save_dialog().await?;
  navigate_to(&pdf_dir).await?;
  confirm_save().await?;

  let saved = wait_for_pdf(&pdf_dir, start).await?;
  rename_to(&saved, &destination).await?;
  return Ok(destination);
}

async fn focus_browser() -> CapabilityErrorResult<String> {
  let browser_id = get_browser_window_id().await?;
  let id = window::focus_window(&browser_id, None).await.map_err(|e| {
    CapabilityError::Browser {
      reason: format!("Browser window could not be focused: {e}"),
    }
  })?;
  return Ok(id);
}

async fn open_print_dialog() -> CapabilityErrorResult<()> {
  keyboard::press_key_on_keyboard("ctrl+p")
    .await
    .map_err(|e| CapabilityError::Browser {
      reason: format!("Could not open the print dialog: {e}"),
    })?;
  delay(Some(PRINT_DIALOG_DELAY_SECS)).await;
  return Ok(());
}

async fn open_save_dialog() -> CapabilityErrorResult<()> {
  press("enter").await?;
  delay(Some(DIALOG_STEP_DELAY_SECS)).await;
  return Ok(());
}

async fn navigate_to(dir: &str) -> CapabilityErrorResult<()> {
  press("ctrl+l").await?;
  type_text(dir).await?;
  delay(Some(DIALOG_STEP_DELAY_SECS)).await;
  press("enter").await?;
  delay(Some(DIALOG_STEP_DELAY_SECS)).await;
  return Ok(());
}

async fn confirm_save() -> CapabilityErrorResult<()> {
  press("enter").await?;
  delay(Some(DIALOG_STEP_DELAY_SECS)).await;
  return Ok(());
}

async fn wait_for_pdf(
  dir: &str,
  start: SystemTime,
) -> CapabilityErrorResult<String> {
  let timeout = Duration::from_secs(SAVE_POLL_TIMEOUT_SECS);
  let interval = Duration::from_millis(SAVE_POLL_INTERVAL_MS);
  let saved = poll(timeout, interval, || newest_pdf(dir, start)).await;
  return match saved {
    Some(path) => Ok(path),
    None => Err(CapabilityError::Browser {
      reason: format!(
        "The PDF was not saved to '{dir}' within {SAVE_POLL_TIMEOUT_SECS} seconds"
      ),
    }),
  };
}

async fn newest_pdf(dir: &str, start: SystemTime) -> Option<String> {
  let mut newest: Option<(SystemTime, String)> = None;
  let entries = read_dir(dir).await.ok()?;
  for (name, _) in entries {
    if !name.ends_with(".pdf") {
      continue;
    }
    let path = Path::new(dir).join(&name);
    let path_str = path.to_string_lossy().to_string();
    let modified = get_modified_time(&path_str)
      .await
      .unwrap_or(SystemTime::UNIX_EPOCH);
    let size = get_file_size(&path_str).await.unwrap_or(0);
    if modified >= start && size > 0 {
      let is_newer =
        newest.as_ref().map(|(m, _)| modified > *m).unwrap_or(true);
      if is_newer {
        newest = Some((modified, path_str));
      }
    }
  }
  return newest.map(|(_, path)| path);
}

async fn pdf_dir() -> CapabilityErrorResult<String> {
  let data_home = xdg::data_home().map_err(|e| CapabilityError::Browser {
    reason: format!("Cannot determine the fenrir data directory: {e}"),
  })?;
  let dir = data_home.join("pdfs").to_string_lossy().to_string();
  create_directory_all(&dir)
    .await
    .map_err(|e| CapabilityError::Browser {
      reason: format!("Cannot create directory '{dir}': {e}"),
    })?;
  return Ok(dir);
}

fn pdf_destination(dir: &str) -> String {
  return Path::new(dir)
    .join(pdf_filename())
    .to_string_lossy()
    .to_string();
}

fn pdf_filename() -> String {
  let timestamp = chrono::Local::now().format("%Y_%m_%d_%H_%M_%S");
  return format!("pdf_{timestamp}.pdf");
}

async fn rename_to(
  saved: &str,
  destination: &str,
) -> CapabilityErrorResult<()> {
  if saved == destination {
    return Ok(());
  }
  return rename_file(saved, destination).await.map_err(|e| {
    CapabilityError::Browser {
      reason: format!(
        "The PDF was saved as '{saved}' but could not be renamed to '{destination}': {e}"
      ),
    }
  });
}

async fn press(key: &str) -> CapabilityErrorResult<()> {
  keyboard::press_key_on_keyboard(key).await.map_err(|e| {
    CapabilityError::Browser {
      reason: format!("Could not press key '{key}': {e}"),
    }
  })?;
  milli_delay(Some(KEY_PACING_MS)).await;
  return Ok(());
}

async fn type_text(text: &str) -> CapabilityErrorResult<()> {
  keyboard::type_with_keyboard(text).await.map_err(|e| {
    CapabilityError::Browser {
      reason: format!("Could not type text '{text}': {e}"),
    }
  })?;
  milli_delay(Some(KEY_PACING_MS)).await;
  return Ok(());
}

async fn get_browser_window_id() -> CapabilityErrorResult<String> {
  let window_ids =
    window::window_ids()
      .await
      .map_err(|e| CapabilityError::Browser {
        reason: e.to_string(),
      })?;
  for window_id in &window_ids {
    let class = window::window_class(window_id).await;
    let Ok(class) = class else {
      continue;
    };
    if is_browser_class(&class) {
      return Ok(window_id.to_string());
    }
  }
  return Err(CapabilityError::Browser {
    reason: "No open browser windows found".to_string(),
  });
}

fn is_browser_class(class: &str) -> bool {
  const BROWSERS: &[&str] = &[
    "chrome",
    "chromium",
    "helium",
    "brave",
    "firefox",
    "librewolf",
  ];
  return BROWSERS.iter().any(|browser| class.contains(browser));
}
