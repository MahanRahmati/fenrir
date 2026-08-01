use ratatui::style::Style;
use ratatui::text::{Line, Span};

pub struct MessageItem {
  text: String,
  reasoning: bool,
}

impl MessageItem {
  pub fn new(text: String, reasoning: bool) -> Self {
    return Self { text, reasoning };
  }

  pub fn to_list_item(&self) -> Line<'static> {
    let mut item = Line::from(Span::raw(self.text.clone()));
    if self.reasoning {
      item = item.style(Style::default().dim());
    }
    return item;
  }

  pub fn is_reasoning(&self) -> bool {
    return self.reasoning;
  }

  pub fn push_string(&mut self, text: &str) {
    self.text.push_str(text);
  }
}
