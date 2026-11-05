use ratatui::buffer::{Buffer, CellWidth};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph, Widget, Wrap};

pub struct Container<'a> {
  title: Option<String>,
  text: Text<'a>,
  color: Option<Color>,
  style: Option<Style>,
  footer: Option<Footer>,
  trim: bool,
  bordered: bool,
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
  style: Option<Style>,
  footer: Option<Footer>,
  trim: bool,
  bordered: bool,
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

  pub fn style(mut self, style: Style) -> Self {
    self.style = Some(style);
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

  pub fn borderless(mut self) -> Self {
    self.bordered = false;
    return self;
  }

  pub fn build(self) -> Container<'a> {
    return Container {
      title: self.title,
      text: self.text,
      color: self.color,
      style: self.style,
      footer: self.footer,
      trim: self.trim,
      bordered: self.bordered,
    };
  }
}

impl<'a> Container<'a> {
  pub fn builder<T>(text: T) -> ContainerBuilder<'a>
  where
    T: Into<Text<'a>>,
  {
    return ContainerBuilder {
      title: None,
      text: text.into(),
      color: None,
      style: None,
      footer: None,
      trim: true,
      bordered: true,
    };
  }

  fn body_lines(&self) -> Vec<Line<'a>> {
    if let Some(style) = self.style {
      return self
        .text
        .lines
        .iter()
        .map(|line| line.clone().patch_style(style))
        .collect();
    }
    return self.text.lines.clone();
  }

  fn title_lines(&self, width: u16) -> Vec<Line<'_>> {
    let title = match &self.title {
      Some(title) => title,
      None => return Vec::new(),
    };
    let mut line = Line::from(title.as_str());
    if let Some(style) = self.style {
      line = line.patch_style(style);
    }
    return vec![line, self.separator(width)];
  }

  fn footer_lines(&self, width: u16) -> Vec<Line<'_>> {
    let footer = match &self.footer {
      Some(footer) => footer,
      None => return Vec::new(),
    };
    let line = match footer.style {
      Some(style) => Line::from(Span::styled(footer.footer.as_str(), style)),
      None => Line::from(footer.footer.as_str()),
    };
    return vec![self.separator(width), line];
  }

  fn paragraph(&self, width: u16) -> Paragraph<'_> {
    let inner_width = if self.bordered {
      width.saturating_sub(2).max(1)
    } else {
      width
    };
    let mut lines = self.title_lines(inner_width);
    lines.extend(self.body_lines());
    lines.extend(self.footer_lines(inner_width));
    let mut item = Paragraph::new(lines).wrap(Wrap { trim: self.trim });
    if self.bordered {
      let mut block = Block::bordered().border_type(BorderType::Rounded);
      if let Some(color) = self.color {
        block = block.border_style(Style::default().fg(color));
      }
      item = item.block(block);
    }
    return item;
  }

  pub fn line_count(&self, width: u16) -> usize {
    return self.paragraph(width).line_count(width);
  }

  pub fn render_lines(&self, width: u16) -> Vec<Line<'static>> {
    let height = self.line_count(width);
    if width == 0 || height == 0 {
      return Vec::new();
    }
    let area = Rect::new(0, 0, width, height as u16);
    let mut buffer = Buffer::empty(area);
    self.paragraph(width).render(area, &mut buffer);
    return buffer_lines(&buffer);
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

fn buffer_lines(buffer: &Buffer) -> Vec<Line<'static>> {
  let mut lines = Vec::with_capacity(buffer.area.height as usize);
  for y in buffer.area.top()..buffer.area.bottom() {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut x = buffer.area.left();
    while x < buffer.area.right() {
      let cell = &buffer[(x, y)];
      let width = cell.symbol().cell_width();
      if width == 0 {
        x += 1;
        continue;
      }
      let style = cell.style();
      if let Some(last) = spans.last_mut()
        && last.style == style
      {
        last.content.to_mut().push_str(cell.symbol());
      } else {
        spans.push(Span::styled(cell.symbol().to_string(), style));
      }
      x += width;
    }
    lines.push(Line::from(spans));
  }
  return lines;
}

impl Widget for Container<'_> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let item = self.paragraph(area.width);
    item.render(area, buf);
  }
}
