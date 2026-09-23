use std::env;

use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::provider_manager::ChatUsage;
use crate::tui::spinner::Spinner;
use crate::utils::usage::format_usage;

enum State {
  Idle,
  Processing(Spinner),
}

pub struct StatusLine {
  cwd: String,
  version: &'static str,
  usage: Option<ChatUsage>,
  state: State,
}

impl StatusLine {
  pub fn new() -> Self {
    let cwd = std::env::current_dir()
      .map(|p| p.display().to_string())
      .unwrap_or_default();
    let version = env!("CARGO_PKG_VERSION");
    return Self {
      cwd,
      version,
      usage: None,
      state: State::Idle,
    };
  }

  pub fn set_usage(&mut self, usage: Option<ChatUsage>) {
    self.usage = usage;
  }

  pub fn set_processing(&mut self, processing: bool) {
    self.state = if processing {
      State::Processing(Spinner::new())
    } else {
      State::Idle
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
    let left_label = match &self.state {
      State::Processing(spinner) => spinner.frame().to_string(),
      State::Idle => Self::to_relative_path(&self.cwd),
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
    let left_len = left_label.chars().count();
    let left_label = if left_len > max_len {
      let start = left_len - max_len;
      let trimmed: String = left_label.chars().skip(start).collect();
      format!("...{trimmed}")
    } else {
      left_label
    };

    let used = left_label.chars().count() + right_label.chars().count() + 1;
    let spacer = w.saturating_sub(used);
    let spans = vec![
      Span::raw(left_label),
      Span::raw(" ".repeat(spacer)),
      Span::raw(right_label),
    ];
    let line = Line::from(spans);
    return Paragraph::new(line);
  }
}
