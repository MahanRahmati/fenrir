use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

pub const MAX_QUEUED_HEIGHT: u16 = 10;

pub struct QueueMessagesList {
  messages: Vec<String>,
  viewport_height: u16,
}

impl QueueMessagesList {
  pub fn new() -> Self {
    return Self {
      messages: Vec::new(),
      viewport_height: 0,
    };
  }

  pub fn is_empty(&self) -> bool {
    return self.messages.is_empty();
  }

  pub fn desired_height(&self, width: u16) -> u16 {
    let inner_width = width.saturating_sub(2).max(1);
    let content = self.content(inner_width);
    let line_count = Paragraph::new(content)
      .wrap(Wrap { trim: true })
      .line_count(inner_width);
    let content_height =
      line_count.min((MAX_QUEUED_HEIGHT - 2) as usize) as u16;
    return content_height + 2;
  }

  pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
    self.viewport_height = area.height.min(MAX_QUEUED_HEIGHT);
    let inner_width = area.width.saturating_sub(2).max(1);
    let height = self.desired_height(area.width);
    let block = Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .border_style(Style::default().fg(Color::Cyan));
    let content = self.content(inner_width);
    let item = Paragraph::new(content)
      .wrap(Wrap { trim: true })
      .block(block);
    let box_area = Rect::new(
      area.x,
      area.y + area.height.saturating_sub(height),
      area.width,
      height,
    );
    frame.render_widget(item, box_area);
  }

  fn content(&self, width: u16) -> Vec<Line<'static>> {
    let mut content: Vec<Line<'static>> = Vec::new();
    let title = "Queued Messages".to_string();
    content.push(Line::from(title));
    content.push(self.separator(width));
    for (i, message) in self.messages.iter().enumerate() {
      content.push(Line::from(message.to_string()));
      if i + 1 < self.messages.len() {
        content.push(self.separator(width));
      }
    }
    return content;
  }

  fn separator(&self, width: u16) -> Line<'static> {
    let separator = "─".repeat(width as usize);
    return Line::from(Span::styled(
      separator,
      Style::default().fg(Color::Cyan),
    ));
  }

  pub fn update_messages(&mut self, queued_messages: Vec<String>) {
    self.messages = queued_messages;
  }
}
