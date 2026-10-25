use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use crate::tui::container::Container;
use crate::tui::{HORIZONTAL_PADDING, TOP_PADDING};
use fenrir_core::tool_manager::task_summary;

const TEXT_EXTRA_PADDING: u16 = 1;

#[derive(PartialEq)]
pub enum Role {
  User,
  Assistant,
  Reasoning,
  SubagentLaunch,
  ToolCall,
  ToolResult,
  Compaction,
}

impl Role {
  fn horizontal_padding(&self) -> u16 {
    return match self {
      Role::User | Role::ToolCall | Role::ToolResult | Role::Compaction => {
        HORIZONTAL_PADDING
      }
      Role::Assistant | Role::Reasoning | Role::SubagentLaunch => {
        HORIZONTAL_PADDING + TEXT_EXTRA_PADDING
      }
    };
  }
}

pub struct ListMessage {
  container: Container<'static>,
  horizontal_padding: u16,
}

impl Widget for ListMessage {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let inner = Rect::new(
      area.x.saturating_add(self.horizontal_padding),
      area.y.saturating_add(TOP_PADDING),
      area.width.saturating_sub(self.horizontal_padding * 2),
      area.height.saturating_sub(TOP_PADDING),
    );
    if inner.width > 0 && inner.height > 0 {
      self.container.render(inner, buf);
    }
  }
}

pub struct MessageItem {
  text: String,
  role: Role,
  error: bool,
  tool_name: Option<String>,
  tool_arguments: Option<String>,
  tool_result: Option<(String, bool)>,
}

impl MessageItem {
  pub fn new(text: String, role: Role) -> Self {
    return Self {
      text,
      role,
      error: false,
      tool_name: None,
      tool_arguments: None,
      tool_result: None,
    };
  }

  pub fn with_error(mut self, error: bool) -> Self {
    self.error = error;
    return self;
  }

  pub fn new_tool_call(name: String, arguments: String) -> Self {
    return Self {
      text: String::new(),
      role: Role::ToolCall,
      error: false,
      tool_name: Some(name),
      tool_arguments: Some(arguments),
      tool_result: None,
    };
  }

  pub fn new_subagent_launch(task: String) -> Self {
    return Self::new(
      format!("Subagent › {}", task_summary(&task)),
      Role::SubagentLaunch,
    );
  }

  pub fn push_tool_result(&mut self, result: String, error: bool) {
    self.tool_result = Some((result, error));
  }

  pub fn is_pending_tool(&self) -> bool {
    return self.role == Role::ToolCall && self.tool_result.is_none();
  }

  pub fn to_list_item(&self, width: u16) -> (ListMessage, u16) {
    let horizontal_padding = self.role.horizontal_padding();
    let width = width.saturating_sub(horizontal_padding * 2);
    let (container, height) = match self.role {
      Role::User => self.user_list_item(width),
      Role::Assistant => self.llm_list_item(width),
      Role::Reasoning => self.reasoning_list_item(width),
      Role::SubagentLaunch => self.subagent_list_item(width),
      Role::ToolCall | Role::ToolResult => self.tool_list_item(width),
      Role::Compaction => self.compaction_list_item(width),
    };
    return (
      ListMessage {
        container,
        horizontal_padding,
      },
      height,
    );
  }

  fn container_height(container: &Container, width: u16) -> u16 {
    return container.line_count(width).max(1) as u16 + TOP_PADDING;
  }

  fn user_list_item(&self, width: u16) -> (Container<'static>, u16) {
    let container = Container::builder(self.text.clone())
      .color(Color::Yellow)
      .build();
    let height = Self::container_height(&container, width);
    return (container, height);
  }

  fn llm_list_item(&self, width: u16) -> (Container<'static>, u16) {
    let container = Container::builder(self.text.clone()).borderless().build();
    let height = Self::container_height(&container, width);
    return (container, height);
  }

  fn reasoning_list_item(&self, width: u16) -> (Container<'static>, u16) {
    let container = Container::builder(self.text.clone())
      .borderless()
      .style(Style::default().dim())
      .build();
    let height = Self::container_height(&container, width);
    return (container, height);
  }

  fn subagent_list_item(&self, width: u16) -> (Container<'static>, u16) {
    let line = self.text.lines().next().unwrap_or("");
    let container = Container::builder(truncate_line(line, width))
      .borderless()
      .style(Style::default().fg(Color::Cyan))
      .build();
    let height = Self::container_height(&container, width);
    return (container, height);
  }

  fn tool_list_item(&self, width: u16) -> (Container<'static>, u16) {
    if self.role == Role::ToolResult {
      let container = Container::builder(self.text.clone())
        .borderless()
        .style(if self.error {
          Style::default().fg(Color::Red)
        } else {
          Style::default().dim()
        })
        .build();
      let height = Self::container_height(&container, width);
      return (container, height);
    }

    let mut title = String::new();
    if let Some(name) = &self.tool_name {
      let trimmed = name.trim();
      if !trimmed.is_empty() {
        title.push(' ');
        title.push_str(trimmed);
      }
      if let Some(arguments) = &self.tool_arguments {
        let args = arguments.trim();
        if !args.is_empty() && args != "{}" {
          if !title.is_empty() {
            title.push(' ');
          }
          title.push_str(args);
        }
      }
    }
    let body: Vec<Line<'static>> = self
      .tool_result
      .as_ref()
      .map(|(result, _)| {
        result.lines().map(|l| Line::from(l.to_string())).collect()
      })
      .unwrap_or_default();
    let mut builder = Container::builder(body).color(Color::Blue);
    if !title.is_empty() {
      builder = builder.title(title);
    }
    if let Some((_, error)) = &self.tool_result {
      builder = builder.style(if *error {
        Style::default().fg(Color::Red)
      } else {
        Style::default().dim()
      });
    }
    let container = builder.build();
    let height = Self::container_height(&container, width);
    return (container, height);
  }

  fn compaction_list_item(&self, width: u16) -> (Container<'static>, u16) {
    let body: Vec<Line<'static>> = self
      .text
      .lines()
      .map(|l| Line::from(l.to_string()))
      .collect();
    let container = Container::builder(body)
      .title(" Compaction")
      .color(Color::Magenta)
      .build();
    let height = Self::container_height(&container, width);
    return (container, height);
  }

  pub fn is_reasoning(&self) -> bool {
    return self.role == Role::Reasoning;
  }

  pub fn is_assistant(&self) -> bool {
    return self.role == Role::Assistant;
  }

  pub fn push_string(&mut self, text: &str) {
    self.text.push_str(text);
  }
}

fn truncate_line(text: &str, width: u16) -> String {
  let width = width as usize;
  let text_len = text.chars().count();
  if text_len <= width {
    return text.to_string();
  }
  if width <= 3 {
    return ".".repeat(width);
  }
  let mut truncated: String = text.chars().take(width - 3).collect();
  truncated.push_str("...");
  return truncated;
}
