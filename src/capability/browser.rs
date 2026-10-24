use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, anyhow, bail};

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

pub async fn open_url(url: &str) -> Result<String> {
  let output = process::run("xdg-open", &[url])
    .await
    .context(format!("Failed to run xdg-open for '{url}'"))?;

  if !output.exit_code.is_success() {
    let stderr = output.stderr.trim();
    let details = if stderr.is_empty() {
      "the URL could not be opened".to_string()
    } else {
      stderr.to_string()
    };
    bail!("{details}");
  }

  delay(Some(DEFAULT_FOCUS_DELAY)).await;
  focus_browser().await?;

  return Ok(format!("Opened {url} in the default browser"));
}

pub async fn save_as_pdf() -> Result<String> {
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

async fn focus_browser() -> Result<String> {
  let browser_id = get_browser_window_id().await?;
  let id = window::focus_window(&browser_id, None)
    .await
    .context("Browser window could not be focused")?;
  return Ok(id);
}

async fn open_print_dialog() -> Result<()> {
  keyboard::press_key_on_keyboard("ctrl+p")
    .await
    .context("Could not open the print dialog")?;
  delay(Some(PRINT_DIALOG_DELAY_SECS)).await;
  return Ok(());
}

async fn open_save_dialog() -> Result<()> {
  press("enter").await?;
  delay(Some(DIALOG_STEP_DELAY_SECS)).await;
  return Ok(());
}

async fn navigate_to(dir: &str) -> Result<()> {
  press("ctrl+l").await?;
  type_text(dir).await?;
  delay(Some(DIALOG_STEP_DELAY_SECS)).await;
  press("enter").await?;
  delay(Some(DIALOG_STEP_DELAY_SECS)).await;
  return Ok(());
}

async fn confirm_save() -> Result<()> {
  press("enter").await?;
  delay(Some(DIALOG_STEP_DELAY_SECS)).await;
  return Ok(());
}

async fn wait_for_pdf(dir: &str, start: SystemTime) -> Result<String> {
  let timeout = Duration::from_secs(SAVE_POLL_TIMEOUT_SECS);
  let interval = Duration::from_millis(SAVE_POLL_INTERVAL_MS);
  let saved = poll(timeout, interval, || newest_pdf(dir, start)).await;
  return match saved {
    Some(path) => Ok(path),
    None => Err(anyhow!(
      "The PDF was not saved to '{dir}' within {SAVE_POLL_TIMEOUT_SECS} seconds"
    )),
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

async fn pdf_dir() -> Result<String> {
  let data_home =
    xdg::data_home().context("Cannot determine the fenrir data directory")?;
  let dir = data_home.join("pdfs").to_string_lossy().to_string();
  create_directory_all(&dir)
    .await
    .context(format!("Cannot create directory '{dir}'"))?;
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

async fn rename_to(saved: &str, destination: &str) -> Result<()> {
  if saved == destination {
    return Ok(());
  }
  return rename_file(saved, destination).await.context(format!(
    "The PDF was saved as '{saved}' but could not be renamed to '{destination}'"
  ));
}

async fn press(key: &str) -> Result<()> {
  keyboard::press_key_on_keyboard(key)
    .await
    .context(format!("Could not press key '{key}'"))?;
  milli_delay(Some(KEY_PACING_MS)).await;
  return Ok(());
}

async fn type_text(text: &str) -> Result<()> {
  keyboard::type_with_keyboard(text)
    .await
    .context(format!("Could not type text '{text}'"))?;
  milli_delay(Some(KEY_PACING_MS)).await;
  return Ok(());
}

async fn get_browser_window_id() -> Result<String> {
  let window_ids = window::window_ids()
    .await
    .context("Failed to list windows")?;
  for window_id in &window_ids {
    let class = window::window_class(window_id).await;
    let Ok(class) = class else {
      continue;
    };
    if is_browser_class(&class) {
      return Ok(window_id.to_string());
    }
  }
  bail!("No open browser windows found");
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
