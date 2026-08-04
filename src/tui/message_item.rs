use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap};

const TOP_PADDING: u16 = 1;
const HORIZONTAL_PADDING: u16 = 2;
const TEXT_EXTRA_PADDING: u16 = 1;

#[derive(PartialEq)]
pub enum Role {
  User,
  Assistant,
  Reasoning,
  ToolCall,
  ToolResult,
}

impl Role {
  fn horizontal_padding(&self) -> u16 {
    return match self {
      Role::User | Role::ToolCall | Role::ToolResult => HORIZONTAL_PADDING,
      Role::Assistant | Role::Reasoning => {
        HORIZONTAL_PADDING + TEXT_EXTRA_PADDING
      }
    };
  }
}

pub struct ListMessage {
  paragraph: Paragraph<'static>,
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
    let horizontal_padding = self.role.horizontal_padding();
    let width = width.saturating_sub(horizontal_padding * 2);
    let (paragraph, height) = match self.role {
      Role::User => self.user_list_item(width),
      Role::Assistant => self.llm_list_item(width),
      Role::Reasoning => self.reasoning_list_item(width),
      Role::ToolCall | Role::ToolResult => self.tool_list_item(width),
    };
    return (
      ListMessage {
        paragraph,
        horizontal_padding,
      },
      height,
    );
  }

  fn wrapped_paragraph(text: String) -> Paragraph<'static> {
    return Paragraph::new(text).wrap(Wrap { trim: true });
  }

  fn item_height(item: &Paragraph, width: u16) -> u16 {
    return item.line_count(width).max(1) as u16 + TOP_PADDING;
  }

  fn user_list_item(&self, width: u16) -> (Paragraph<'static>, u16) {
    let block = Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .border_style(Style::default().fg(Color::Yellow));
    let item = Self::wrapped_paragraph(self.text.clone()).block(block);
    let height = Self::item_height(&item, width);
    return (item, height);
  }

  fn llm_list_item(&self, width: u16) -> (Paragraph<'static>, u16) {
    let item = Self::wrapped_paragraph(self.text.clone());
    let height = Self::item_height(&item, width);
    return (item, height);
  }

  fn reasoning_list_item(&self, width: u16) -> (Paragraph<'static>, u16) {
    let item =
      Self::wrapped_paragraph(self.text.clone()).style(Style::default().dim());
    let height = Self::item_height(&item, width);
    return (item, height);
  }

  fn tool_list_item(&self, width: u16) -> (Paragraph<'static>, u16) {
    if self.role == Role::ToolResult {
      let mut item =
        Paragraph::new(self.text.clone()).wrap(Wrap { trim: true });
      item = item.style(if self.error {
        Style::default().fg(Color::Red)
      } else {
        Style::default().dim()
      });
      let height = item.line_count(width).max(1) as u16 + TOP_PADDING;
      return (item, height);
    }

    let block = Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .border_style(Style::default().fg(Color::Blue));
    let mut content: Vec<Line<'static>> = Vec::new();
    if let Some(name) = &self.tool_name {
      let trimmed = name.trim();
      let mut title = String::new();
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
      if !title.is_empty() {
        let separator = "─".repeat(width.saturating_sub(2) as usize);
        content.push(Line::from(title));
        content.push(Line::from(Span::styled(
          separator,
          Style::default().fg(Color::Blue),
        )));
      }
    }
    if let Some((result, _)) = &self.tool_result {
      content.extend(result.lines().map(|l| Line::from(l.to_string())));
    }
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
    return (item, height);
  }

  pub fn is_reasoning(&self) -> bool {
    return self.role == Role::Reasoning;
  }

  pub fn push_string(&mut self, text: &str) {
    self.text.push_str(text);
  }
}
