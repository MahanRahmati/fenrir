use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph, Widget, Wrap};

pub struct Container<'a> {
  title: Option<String>,
  text: Text<'a>,
  color: Option<Color>,
}

impl<'a> Container<'a> {
  pub fn new<T>(title: Option<String>, text: T, color: Option<Color>) -> Self
  where
    T: Into<Text<'a>>,
  {
    let text: Text = text.into();
    return Self { title, text, color };
  }
}

impl Widget for Container<'_> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let mut block = Block::bordered().border_type(BorderType::Rounded);
    if let Some(color) = self.color {
      block = block.border_style(Style::default().fg(color));
    }
    let mut lines = Vec::new();
    if let Some(title) = self.title {
      let line = Line::from(title.to_string());
      lines.push(line);
      let inner_width = area.width.saturating_sub(2).max(1);
      let separator = separator(self.color, inner_width);
      lines.push(separator);
    }
    lines.extend(self.text.lines);
    let item = Paragraph::new(lines).wrap(Wrap { trim: true }).block(block);
    item.render(area, buf);
  }
}

fn separator(color: Option<Color>, width: u16) -> Line<'static> {
  let separator = "─".repeat(width as usize);
  let mut style = Style::default();
  if let Some(color) = color {
    style = style.fg(color);
  }
  let line = Line::from(Span::styled(separator, style));
  return line;
}
