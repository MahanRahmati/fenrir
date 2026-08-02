use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

const TOP_PADDING: u16 = 1;

#[derive(PartialEq)]
pub enum Role {
  User,
  Assistant,
  Reasoning,
  ToolCall,
  ToolResult,
}

pub struct ListMessage {
  paragraph: Paragraph<'static>,
}

impl Widget for ListMessage {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let inner = Rect::new(
      area.x,
      area.y + TOP_PADDING,
      area.width,
      area.height.saturating_sub(TOP_PADDING),
    );
    if inner.height > 0 {
      self.paragraph.render(inner, buf);
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

  pub fn push_tool_result(&mut self, result: String, error: bool) {
    self.tool_result = Some((result, error));
  }

  pub fn is_pending_tool(&self) -> bool {
    return self.role == Role::ToolCall && self.tool_result.is_none();
  }

  pub fn to_list_item(&self, width: u16) -> (ListMessage, u16) {
    return match self.role {
      Role::User => self.user_list_item(width),
      Role::Assistant => self.llm_list_item(width),
      Role::Reasoning => self.reasoning_list_item(width),
      Role::ToolCall | Role::ToolResult => self.tool_list_item(width),
    };
  }

  fn user_list_item(&self, width: u16) -> (ListMessage, u16) {
    let block = Block::default()
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Yellow));
    let item = Paragraph::new(self.text.clone())
      .wrap(Wrap { trim: true })
      .block(block);
    let height = item.line_count(width).max(1) as u16 + TOP_PADDING;
    return (ListMessage { paragraph: item }, height);
  }

  fn llm_list_item(&self, width: u16) -> (ListMessage, u16) {
    let item = Paragraph::new(self.text.clone()).wrap(Wrap { trim: true });
    let height = item.line_count(width).max(1) as u16 + TOP_PADDING;
    return (ListMessage { paragraph: item }, height);
  }

  fn reasoning_list_item(&self, width: u16) -> (ListMessage, u16) {
    let item = Paragraph::new(self.text.clone())
      .wrap(Wrap { trim: true })
      .style(Style::default().dim());
    let height = item.line_count(width).max(1) as u16 + TOP_PADDING;
    return (ListMessage { paragraph: item }, height);
  }

  fn tool_list_item(&self, width: u16) -> (ListMessage, u16) {
    if self.role == Role::ToolResult {
      let mut item =
        Paragraph::new(self.text.clone()).wrap(Wrap { trim: true });
      item = item.style(if self.error {
        Style::default().fg(Color::Red)
      } else {
        Style::default().dim()
      });
      let height = item.line_count(width).max(1) as u16 + TOP_PADDING;
      return (ListMessage { paragraph: item }, height);
    }

    let mut block = Block::default()
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Blue));
    if let Some(name) = &self.tool_name {
      block = block.title(name.clone());
    }
    let mut content_parts = Vec::new();
    if let Some(arguments) = &self.tool_arguments {
      let trimmed = arguments.trim();
      if !trimmed.is_empty() && trimmed != "{}" {
        content_parts.push(arguments.clone());
      }
    }
    if let Some((result, _)) = &self.tool_result {
      content_parts.push(result.clone());
    }
    let content = content_parts.join("\n\n");
    let mut item = Paragraph::new(content)
      .wrap(Wrap { trim: true })
      .block(block);
    if let Some((_, error)) = &self.tool_result {
      item = item.style(if *error {
        Style::default().fg(Color::Red)
      } else {
        Style::default().dim()
      });
    }
    let height = item.line_count(width).max(1) as u16 + TOP_PADDING;
    return (ListMessage { paragraph: item }, height);
  }

  pub fn is_reasoning(&self) -> bool {
    return self.role == Role::Reasoning;
  }

  pub fn push_string(&mut self, text: &str) {
    self.text.push_str(text);
  }
}
