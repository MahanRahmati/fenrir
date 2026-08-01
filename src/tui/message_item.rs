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
}

impl MessageItem {
  pub fn new(text: String, role: Role) -> Self {
    return Self { text, role };
  }

  pub fn to_list_item(&self, width: u16) -> (ListMessage, u16) {
    let mut block = Block::default();
    let style = Style::default();
    if self.role == Role::User {
      block = block
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    }
    let mut item = Paragraph::new(self.text.clone())
      .wrap(Wrap { trim: true })
      .block(block)
      .style(style);
    if self.role == Role::Reasoning {
      item = item.style(Style::default().dim());
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
