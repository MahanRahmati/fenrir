use std::env;

use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::provider_manager::ChatUsage;
use crate::utils::usage::format_usage;

pub struct StatusLine {
  cwd: String,
  version: &'static str,
  usage: Option<ChatUsage>,
  processing: bool,
}

impl StatusLine {
  pub fn new(usage: Option<ChatUsage>, processing: bool) -> Self {
    let cwd = std::env::current_dir()
      .map(|p| p.display().to_string())
      .unwrap_or_default();
    let version = env!("CARGO_PKG_VERSION");
    return Self {
      cwd,
      version,
      usage,
      processing,
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

  fn left_label(&self) -> String {
    let left_label = if self.processing {
      "Processing...".to_string()
    } else {
      Self::to_relative_path(&self.cwd)
    };
    return left_label;
  }

  fn right_label(&self) -> String {
    let usage_display = self.usage.and_then(|u| format_usage(u.prompt_tokens));
    let version_display = format!("v{}", self.version);
    let mut right_label = String::new();
    if let Some(usage) = usage_display {
      right_label = format!("{usage} ");
    }
    right_label = format!("{right_label}{version_display}");
    return right_label;
  }

  pub fn draw(&self, width: u16) -> Paragraph<'_> {
    let left_label = self.left_label();
    let right_label = self.right_label();
    let w = width as usize;
    let max_len = w.saturating_sub(4).max(8);
    let left_label = if left_label.len() > max_len {
      let start = left_label.len() - max_len;
      format!("...{}", &left_label[start..])
    } else {
      left_label
    };

    let spacer = w.saturating_sub(left_label.len() + right_label.len() + 1);
    let spans = vec![
      Span::raw(left_label),
      Span::raw(" ".repeat(spacer)),
      Span::raw(right_label),
    ];
    let line = Line::from(spans);
    return Paragraph::new(line);
  }
}
