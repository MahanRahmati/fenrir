use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget};

const PADDING: u16 = 1;

pub struct Button<'a> {
  label: &'a str,
  selected: bool,
}

impl<'a> Button<'a> {
  pub fn new(label: &'a str) -> Self {
    return Self {
      label,
      selected: false,
    };
  }

  pub fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    return self;
  }

  pub fn width(&self) -> u16 {
    return self.label.chars().count() as u16 + PADDING * 2 + 2;
  }
}

impl Widget for Button<'_> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let style = if self.selected {
      Style::default().reversed()
    } else {
      Style::default()
    };
    let block = Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded);
    Paragraph::new(Span::styled(self.label, style))
      .alignment(Alignment::Center)
      .block(block)
      .render(area, buf);
  }
}
