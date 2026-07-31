use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

pub struct MessageItem {
  text: String,
  reasoning: bool,
}

impl MessageItem {
  pub fn new(text: String, reasoning: bool) -> Self {
    return Self { text, reasoning };
  }

  pub fn to_list_item(&self) -> ListItem<'_> {
    let content = Line::from(Span::raw(self.text.clone()));
    let mut item = ListItem::new(content);
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
