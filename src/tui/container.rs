use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph, Widget, Wrap};

pub struct Container<'a> {
  title: Option<String>,
  text: Text<'a>,
  color: Option<Color>,
  footer: Option<Footer>,
  trim: bool,
}

pub struct Footer {
  footer: String,
  style: Option<Style>,
}

impl Footer {
  pub fn new(footer: String, style: Option<Style>) -> Self {
    return Self { footer, style };
  }
}

pub struct ContainerBuilder<'a> {
  title: Option<String>,
  text: Text<'a>,
  color: Option<Color>,
  footer: Option<Footer>,
  trim: bool,
}

impl<'a> ContainerBuilder<'a> {
  pub fn title<T: Into<String>>(mut self, title: T) -> Self {
    self.title = Some(title.into());
    return self;
  }

  pub fn color(mut self, color: Color) -> Self {
    self.color = Some(color);
    return self;
  }

  pub fn footer(mut self, footer: Footer) -> Self {
    self.footer = Some(footer);
    return self;
  }

  pub fn trim(mut self, trim: bool) -> Self {
    self.trim = trim;
    return self;
  }

  pub fn build(self) -> Container<'a> {
    return Container {
      title: self.title,
      text: self.text,
      color: self.color,
      footer: self.footer,
      trim: self.trim,
    };
  }
}

impl<'a> Container<'a> {
  pub fn new<T>(text: T) -> ContainerBuilder<'a>
  where
    T: Into<Text<'a>>,
  {
    return ContainerBuilder {
      title: None,
      text: text.into(),
      color: None,
      footer: None,
      trim: true,
    };
  }

  fn paragraph(&self, width: u16) -> Paragraph<'_> {
    let mut block = Block::bordered().border_type(BorderType::Rounded);
    if let Some(color) = self.color {
      block = block.border_style(Style::default().fg(color));
    }
    let mut lines = Vec::new();
    let inner_width = width.saturating_sub(2).max(1);
    if let Some(title) = &self.title {
      let line = Line::from(title.as_str());
      lines.push(line);
      let separator = self.separator(inner_width);
      lines.push(separator);
    }
    lines.extend(self.text.lines.clone());
    if let Some(footer) = &self.footer {
      let separator = self.separator(inner_width);
      lines.push(separator);
      let line = match footer.style {
        Some(style) => Line::from(Span::styled(footer.footer.as_str(), style)),
        None => Line::from(footer.footer.as_str()),
      };
      lines.push(line);
    }
    let item = Paragraph::new(lines)
      .wrap(Wrap { trim: self.trim })
      .block(block);
    return item;
  }

  fn separator(&self, width: u16) -> Line<'static> {
    let separator = "─".repeat(width as usize);
    let mut style = Style::default();
    if let Some(color) = self.color {
      style = style.fg(color);
    }
    let line = Line::from(Span::styled(separator, style));
    return line;
  }
}

impl Widget for Container<'_> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let item = self.paragraph(area.width);
    item.render(area, buf);
  }
}
