use std::env;

use ratatui::{
  text::{Line, Span},
  widgets::Paragraph,
};

pub struct StatusLine {
  cwd: String,
  version: String,
}

impl StatusLine {
  pub fn new() -> Self {
    let cwd = std::env::current_dir()
      .map(|p| p.display().to_string())
      .unwrap_or_default();
    let version = env!("CARGO_PKG_VERSION").to_string();
    return Self { cwd, version };
  }

  fn to_relative_path(path: &str) -> String {
    if let Some(home) = env::home_dir() {
      if let Some(home_str) = home.to_str() {
        if path.starts_with(home_str) {
          return format!("~{}", &path[home_str.len()..]);
        }
      }
    }
    path.to_string()
  }

  pub fn draw(&self, width: u16) -> Paragraph<'_> {
    let cwd_display = Self::to_relative_path(&self.cwd);
    let version_display = format!("v{}", self.version);

    let w = width as usize;
    let max_len = w.saturating_sub(4).max(8);
    let cwd_width = if cwd_display.len() > max_len {
      let start = cwd_display.len() - max_len;
      format!("...{}", &cwd_display[start..])
    } else {
      cwd_display
    };

    let spacer = w.saturating_sub(cwd_width.len() + version_display.len() + 1);

    let line = Line::from(vec![
      Span::raw(cwd_width),
      Span::raw(" ".repeat(spacer)),
      Span::raw(version_display),
    ]);

    Paragraph::new(line)
  }
}
