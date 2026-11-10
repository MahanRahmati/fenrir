use std::env;

use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::spinner::Spinner;
use fenrir_core::provider_manager::ChatUsage;
use fenrir_core::utils::usage::format_usage;

enum State {
  Idle,
  Processing(Spinner),
}

struct RetryStatus {
  attempt: u32,
  max_attempts: u32,
  delay_seconds: u64,
}

pub struct StatusLine {
  cwd: String,
  version: &'static str,
  usage: Option<ChatUsage>,
  context_window: Option<u64>,
  subagents: usize,
  retry: Option<RetryStatus>,
  state: State,
}

impl StatusLine {
  pub fn new(cwd: &str) -> Self {
    let version = env!("CARGO_PKG_VERSION");
    return Self {
      cwd: cwd.to_owned(),
      version,
      usage: None,
      context_window: None,
      subagents: 0,
      retry: None,
      state: State::Idle,
    };
  }

  pub fn set_usage(
    &mut self,
    usage: Option<ChatUsage>,
    context_window: Option<u64>,
  ) {
    self.usage = usage;
    self.context_window = context_window;
  }

  pub fn update_subagents(&mut self, delta: i32) {
    self.subagents = if delta >= 0 {
      self.subagents.saturating_add(delta as usize)
    } else {
      self.subagents.saturating_sub(delta.unsigned_abs() as usize)
    };
  }

  pub fn reset_subagents(&mut self) {
    self.subagents = 0;
  }

  pub fn set_processing(&mut self, processing: bool) {
    self.state = if processing {
      State::Processing(Spinner::new())
    } else {
      State::Idle
    };
    self.retry = None;
  }

  pub fn set_retrying(
    &mut self,
    attempt: u32,
    max_attempts: u32,
    delay_seconds: u64,
  ) {
    self.retry = Some(RetryStatus {
      attempt,
      max_attempts,
      delay_seconds,
    });
  }

  pub fn clear_retrying(&mut self) {
    self.retry = None;
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
      State::Processing(spinner) => match &self.retry {
        Some(retry) => format!(
          "{} retrying {}/{} in {}s",
          spinner.frame(),
          retry.attempt,
          retry.max_attempts,
          retry.delay_seconds
        ),
        None => spinner.frame().to_string(),
      },
      State::Idle => Self::to_relative_path(&self.cwd),
    };
    return left_label;
  }

  fn right_label(&self) -> String {
    let usage_display = self
      .usage
      .and_then(|u| format_usage(u.prompt_tokens, self.context_window));
    let version_display = format!("v{}", self.version);
    let mut right_label = String::new();
    if self.subagents > 0 {
      let label = if self.subagents == 1 {
        "1 Subagent".to_string()
      } else {
        format!("{} Subagents", self.subagents)
      };
      right_label = format!("{label} ");
    }
    if let Some(usage) = usage_display {
      right_label = format!("{right_label}{usage} ");
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
