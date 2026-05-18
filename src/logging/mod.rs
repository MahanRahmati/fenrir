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

use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Sets the global verbose flag.
///
/// This should be called once at application startup, typically from
/// main.rs after parsing CLI arguments.
///
/// # Arguments
///
/// * `value` - Whether to enable verbose output
pub fn set_verbose(value: bool) {
  VERBOSE.store(value, Ordering::Relaxed);
}

/// Checks if verbose mode is enabled.
///
/// # Returns
///
/// `true` if verbose output is enabled, `false` otherwise.
pub fn is_verbose() -> bool {
  return VERBOSE.load(Ordering::Relaxed);
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
        if $crate::logging::is_verbose() {
            let now = chrono::Local::now();
            println!("[{}] {}", now.format("%H:%M:%S"), format!($($arg)*));
        }
    };
}
