use std::env;

use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::provider_manager::ChatUsage;

pub struct StatusLine {
  cwd: String,
  version: &'static str,
  usage: Option<ChatUsage>,
}

impl StatusLine {
  pub fn new(usage: Option<ChatUsage>) -> Self {
    let cwd = std::env::current_dir()
      .map(|p| p.display().to_string())
      .unwrap_or_default();
    let version = env!("CARGO_PKG_VERSION");
    return Self {
      cwd,
      version,
      usage,
    };
  }

  fn to_relative_path(path: &str) -> String {
    if let Some(home) = env::home_dir()
      && let Some(home_str) = home.to_str()
      && path.starts_with(home_str)
    {
      return format!("~{}", &path[home_str.len()..]);
    }
    return path.to_string();
  }

  fn format_usage(tokens: u64) -> Option<String> {
    if tokens < 1000 {
      return Some(tokens.to_string());
    }
    return Some(format!("{:.1}K", tokens as f64 / 1000.0));
  }

  pub fn draw(&self, width: u16) -> Paragraph<'_> {
    let cwd_display = Self::to_relative_path(&self.cwd);
    let version_display = format!("v{}", self.version);
    let usage_display =
      self.usage.and_then(|u| Self::format_usage(u.prompt_tokens));
    let w = width as usize;
    let max_len = w.saturating_sub(4).max(8);
    let cwd_width = if cwd_display.len() > max_len {
      let start = cwd_display.len() - max_len;
      format!("...{}", &cwd_display[start..])
    } else {
      cwd_display
    };

    let spacer = w.saturating_sub(
      cwd_width.len()
        + usage_display.as_ref().map_or(0, |u| u.len() + 1)
        + version_display.len()
        + 1,
    );

    let mut spans = vec![Span::raw(cwd_width), Span::raw(" ".repeat(spacer))];
    if let Some(usage) = usage_display {
      spans.push(Span::raw(usage));
      spans.push(Span::raw(" "));
    }
    spans.push(Span::raw(version_display));
    let line = Line::from(spans);

    return Paragraph::new(line);
  }
}
