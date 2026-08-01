use ratatui::style::Style;
use ratatui::widgets::{Paragraph, Wrap};

pub struct MessageItem {
  text: String,
  reasoning: bool,
}

impl MessageItem {
  pub fn new(text: String, reasoning: bool) -> Self {
    return Self { text, reasoning };
  }

  pub fn to_list_item(&self, width: u16) -> (Paragraph<'static>, u16) {
    let mut item = Paragraph::new(self.text.clone()).wrap(Wrap { trim: true });
    if self.reasoning {
      item = item.style(Style::default().dim());
    }
    let height = item.line_count(width).max(1) as u16;
    return (item, height);
  }

  pub fn is_reasoning(&self) -> bool {
    return self.reasoning;
  }

  pub fn push_string(&mut self, text: &str) {
    self.text.push_str(text);
  }
}
