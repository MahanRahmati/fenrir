use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use crate::tui::container::Container;
use crate::tui::tool_view::{ToolCallView, tool_call_view, tool_result_label};
use crate::tui::{HORIZONTAL_PADDING, TOP_PADDING};
use fenrir_core::tool_manager::{
  ToolPreview, ToolPreviewLine, ToolPreviewLineKind,
};

const TEXT_EXTRA_PADDING: u16 = 1;

#[derive(PartialEq)]
pub enum Role {
  User,
  Assistant,
  Reasoning,
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
      Role::Assistant | Role::Reasoning => {
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
  tool_call_id: Option<String>,
  tool_name: Option<String>,
  tool_view: ToolCallView,
  tool_result: Option<(String, bool)>,
}

impl MessageItem {
  pub fn new(text: String, role: Role) -> Self {
    return Self {
      text,
      role,
      error: false,
      tool_call_id: None,
      tool_name: None,
      tool_view: ToolCallView::default(),
      tool_result: None,
    };
  }

  pub fn with_error(mut self, error: bool) -> Self {
    self.error = error;
    return self;
  }

  pub fn new_tool_call(
    id: String,
    name: String,
    arguments: String,
    preview: Option<ToolPreview>,
  ) -> Self {
    let tool_view = tool_call_view(&name, &arguments, preview.as_ref());
    return Self {
      text: String::new(),
      role: Role::ToolCall,
      error: false,
      tool_call_id: Some(id),
      tool_name: Some(name),
      tool_view,
      tool_result: None,
    };
  }

  pub fn push_tool_result(&mut self, result: String, error: bool) {
    self.tool_result = Some((result, error));
  }

  pub fn is_pending_tool_call(&self, id: &str) -> bool {
    return self.role == Role::ToolCall
      && self.tool_call_id.as_deref() == Some(id)
      && self.tool_result.is_none();
  }

  pub fn to_list_item(&self, width: u16) -> (ListMessage, u16) {
    let horizontal_padding = if self.role == Role::ToolCall {
      self.role.horizontal_padding() + TEXT_EXTRA_PADDING
    } else {
      self.role.horizontal_padding()
    };
    let width = width.saturating_sub(horizontal_padding * 2);
    let (container, height) = match self.role {
      Role::User => self.user_list_item(width),
      Role::Assistant => self.llm_list_item(width),
      Role::Reasoning => self.reasoning_list_item(width),
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

    let view = &self.tool_view;
    let label = match (&self.tool_name, &self.tool_result) {
      (Some(name), Some((result, false))) => {
        tool_result_label(name, view, result)
          .unwrap_or_else(|| view.label.clone())
      }
      _ => view.label.clone(),
    };
    let label = format!("{} {}", view.icon, label);
    let label = Line::from(truncate_line(&label, width));
    let mut lines: Vec<Line<'static>> = vec![label];
    lines.extend(view.body.iter().map(preview_line));
    if let Some((result, error)) = &self.tool_result
      && (view.show_result || *error)
    {
      let style = if *error {
        Style::default().fg(Color::Red)
      } else {
        Style::default().dim()
      };
      lines.extend(
        result
          .lines()
          .map(|line| Line::from(line.to_string()).patch_style(style)),
      );
    }
    let container = Container::builder(lines).borderless().trim(false).build();
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

fn preview_line(line: &ToolPreviewLine) -> Line<'static> {
  let style = match line.kind {
    ToolPreviewLineKind::Added => Style::default().fg(Color::Green),
    ToolPreviewLineKind::Removed => Style::default().fg(Color::Red),
    ToolPreviewLineKind::Header => Style::default().fg(Color::Cyan),
    ToolPreviewLineKind::Warning => Style::default().fg(Color::Yellow),
    ToolPreviewLineKind::Plain | ToolPreviewLineKind::Context => {
      Style::default().dim()
    }
  };
  return Line::from(line.text.clone()).patch_style(style);
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
