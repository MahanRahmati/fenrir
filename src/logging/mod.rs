//! Global logging module with verbose flag support.
//!
//! Provides a simple global verbose flag and macro for conditional logging
//! with timestamps. All modules can use the [`vlog!`] macro to print
//! verbose messages without passing verbose flags through function signatures.
//!
//! ## Components
//!
//! - [`set_verbose`]: Set the global verbose flag at application startup
//! - [`is_verbose`]: Check if verbose mode is enabled
//! - [`vlog!`]: Macro for printing timestamped verbose messages
//!
//! ## Usage
//!
//! ```rust
//! // In main.rs, set verbose from CLI args:
//! set_verbose(cli.verbose);
//!
//! // Anywhere in the codebase:
//! vlog!("Hello World!");
//! vlog!("Hello {}", world);
//! ```

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::utils::xdg::place_state_file;

static VERBOSE: AtomicBool = AtomicBool::new(false);
static PRINT_ENABLED: AtomicBool = AtomicBool::new(false);
static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

/// Sets the global verbose flag.
///
/// This should be called once at application startup, typically from
/// main.rs after parsing CLI arguments.
///
/// # Arguments
///
/// * `value` - Whether to enable verbose output
pub(crate) fn set_verbose(value: bool) {
  VERBOSE.store(value, Ordering::Relaxed);
}

/// Checks if verbose mode is enabled.
///
/// # Returns
///
/// `true` if verbose output is enabled, `false` otherwise.
pub(crate) fn is_verbose() -> bool {
  return VERBOSE.load(Ordering::Relaxed);
}

/// Sets whether printing to stdout is enabled.
///
/// When disabled, the [`vlog!`] macro will skip `println!` but
/// will still write to the log file.
///
/// # Arguments
///
/// * `value` - Whether to enable stdout printing
pub(crate) fn set_print_enabled(value: bool) {
  PRINT_ENABLED.store(value, Ordering::Relaxed);
}

/// Checks if stdout printing is enabled.
///
/// # Returns
///
/// `true` if printing to stdout is enabled, `false` otherwise.
pub(crate) fn is_print_enabled() -> bool {
  return PRINT_ENABLED.load(Ordering::Relaxed);
}

/// Sets the global log file handle.
///
/// Stores the provided `File` in the `LOG_FILE` static so that
/// [`write_to_log_file`] can write to it later.
///
/// # Arguments
///
/// * `file` - The file for the log messages.
///
/// # Errors
///
/// Returns an error if the mutex is not locked.
pub(crate) fn set_log_file(file: File) -> Result<(), String> {
  let mut guard = LOG_FILE
    .lock()
    .map_err(|e| format!("Failed to lock log file mutex: {e}"))?;
  *guard = Some(file);
  return Ok(());
}

/// Initializes the log file for the application.
///
/// Creates a timestamped log file in the XDG state directory and stores
/// it in the global `LOG_FILE` static. Called during application startup.
///
/// # Errors
///
/// Returns an error if the log file cannot be created or opened.
pub(crate) fn init_file_logging() -> Result<(), String> {
  let now = chrono::Local::now();
  let timestamp = now.format("%Y%m%d_%H%M%S");
  let filename = format!("fenrir_{timestamp}.log");
  let path = place_state_file(&filename)
    .map_err(|e| format!("Failed to create log file: {e}"))?;
  let file = OpenOptions::new()
    .create(true)
    .append(true)
    .open(&path)
    .map_err(|e| {
      format!("Failed to open log file '{}': {e}", path.display())
    })?;
  set_log_file(file)?;
  return Ok(());
}

/// Sets up the logging system.
///
/// Configures the verbose flag and initializes the log file.
/// This should be called once at application startup.
///
/// # Arguments
///
/// * `value` - Whether to enable verbose output
///
/// # Errors
///
/// Returns an error if the log file cannot be initialized.
pub fn setup_logging(value: bool) -> Result<(), String> {
  set_verbose(value);
  init_file_logging()?;
  return Ok(());
}

/// Writes a message to the log file.
///
/// Acquires the `LOG_FILE` mutex and writes the message as a line.
/// Silently ignores write errors and poisoned mutexes.
///
/// # Arguments
///
/// * `msg` - The message to be written.
pub(crate) fn write_to_log_file(msg: String) {
  if let Ok(mut guard) = LOG_FILE.lock()
    && let Some(file) = &mut *guard
  {
    let _ = writeln!(file, "{msg}");
  };
}

/// Prints a verbose message with timestamp if verbose mode is enabled.
///
/// Messages are prefixed with the current time in HH:MM:SS format.
/// If verbose mode is disabled, this macro does nothing.
///
/// # Examples
///
/// ```rust
/// vlog!("Hello World");
/// vlog!("Hello {}", world);
/// ```
#[macro_export]
macro_rules! vlog {
    ($($arg:tt)*) => {
        let now = chrono::Local::now();
        let timestamp = now.format("%H:%M:%S");
        let msg = format!("[{timestamp}] {}", format!($($arg)*));
        if $crate::logging::is_verbose() {
            if $crate::logging::is_print_enabled() {
                println!("{msg}");
            }
        }
        $crate::logging::write_to_log_file(msg);
    };
}
