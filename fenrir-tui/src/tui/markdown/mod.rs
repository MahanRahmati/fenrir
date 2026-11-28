mod parse;
mod wrap;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use self::parse::{Alignment, Block, Inline, List, ListItem, Table};
use super::icon::*;

pub fn render(markdown: &str, width: u16) -> Vec<Line<'static>> {
  let width = width.max(1) as usize;
  let mut lines = Vec::new();
  render_blocks(&parse::blocks(markdown), width, &mut lines);
  trim_blank_lines(&mut lines);
  return lines;
}

pub struct Renderer {
  width: usize,
  blocks: Vec<Block>,
  lines: Vec<Vec<Line<'static>>>,
}

impl Renderer {
  pub fn new() -> Self {
    return Self {
      width: 0,
      blocks: Vec::new(),
      lines: Vec::new(),
    };
  }

  pub fn clear(&mut self) {
    self.blocks.clear();
    self.lines.clear();
  }

  pub fn render(&mut self, markdown: &str, width: u16) -> Vec<Line<'static>> {
    let width = width.max(1) as usize;
    if self.width != width {
      self.width = width;
      self.clear();
    }
    let blocks = parse::blocks(markdown);
    let shared = self
      .blocks
      .iter()
      .zip(blocks.iter())
      .take_while(|(cached, block)| *cached == *block)
      .count();
    self.blocks.truncate(shared);
    self.lines.truncate(shared);
    for block in &blocks[shared..] {
      let mut block_lines = Vec::new();
      render_blocks(std::slice::from_ref(block), width, &mut block_lines);
      self.blocks.push(block.clone());
      self.lines.push(block_lines);
    }
    let mut lines: Vec<Line<'static>> =
      self.lines.iter().flatten().cloned().collect();
    trim_blank_lines(&mut lines);
    return lines;
  }
}

impl Default for Renderer {
  fn default() -> Self {
    return Self::new();
  }
}

fn trim_blank_lines(lines: &mut Vec<Line<'static>>) {
  while lines.first().is_some_and(is_blank_line) {
    lines.remove(0);
  }
  while lines.last().is_some_and(is_blank_line) {
    lines.pop();
  }
  if lines.is_empty() {
    lines.push(Line::default());
  }
}

fn render_blocks(
  blocks: &[Block],
  width: usize,
  lines: &mut Vec<Line<'static>>,
) {
  for block in blocks {
    match block {
      Block::Heading { level, inlines } => {
        let segments = segments(inlines, heading_style(*level));
        lines.extend(wrap::wrap(segments, width));
      }
      Block::Paragraph(inlines) => {
        let segments = segments(inlines, Style::default());
        lines.extend(wrap::wrap(segments, width));
      }
      Block::Code { lang, text } => {
        render_code(lang.as_deref(), text, width, lines);
      }
      Block::List(list) => render_list(list, width, lines),
      Block::Table(table) => render_table(table, width, lines),
      Block::Quote(blocks) => render_quote(blocks, width, lines),
      Block::Rule => lines.push(rule_line(width)),
      Block::Blank => lines.push(Line::default()),
    }
  }
}

fn render_list(list: &List, width: usize, lines: &mut Vec<Line<'static>>) {
  let markers: Vec<(String, Style)> = list
    .items
    .iter()
    .enumerate()
    .map(|(index, item)| list_marker(list, index, item))
    .collect();
  let marker_width = markers
    .iter()
    .map(|(marker, _)| wrap::width(marker))
    .max()
    .unwrap_or(0);
  let content_width = width.saturating_sub(marker_width).max(1);
  for (item, (marker, marker_style)) in list.items.iter().zip(markers) {
    let mut item_lines = Vec::new();
    render_blocks(&item.blocks, content_width, &mut item_lines);
    if item_lines.is_empty() {
      item_lines.push(Line::default());
    }
    let marker_padding = marker_width - wrap::width(&marker);
    for (line_index, line) in item_lines.into_iter().enumerate() {
      let mut spans = Vec::with_capacity(line.spans.len() + 1);
      if line_index == 0 {
        spans.push(Span::styled(marker.clone(), marker_style));
        if marker_padding > 0 {
          spans.push(Span::raw(" ".repeat(marker_padding)));
        }
      } else if !line.spans.is_empty() {
        spans.push(Span::raw(" ".repeat(marker_width)));
      }
      spans.extend(line.spans);
      lines.push(Line::from(spans));
    }
  }
}

fn list_marker(list: &List, index: usize, item: &ListItem) -> (String, Style) {
  if let Some(checked) = item.task {
    return match checked {
      true => (
        format!("{TASK_DONE_ICON} "),
        Style::default().fg(Color::Green),
      ),
      false => (
        format!("{TASK_TODO_ICON} "),
        Style::default().fg(Color::DarkGray),
      ),
    };
  }

  if list.ordered {
    let number = list.start + index as u64;
    return (format!("{number}. "), Style::default().fg(Color::DarkGray));
  }
  return (
    format!("{BULLET_ICON} "),
    Style::default().fg(Color::DarkGray),
  );
}

const QUOTE_PREFIX_WIDTH: usize = 2;

fn render_quote(
  blocks: &[Block],
  width: usize,
  lines: &mut Vec<Line<'static>>,
) {
  let inner = width.saturating_sub(QUOTE_PREFIX_WIDTH).max(1);
  let mut quote_lines = Vec::new();
  render_blocks(blocks, inner, &mut quote_lines);
  let style = Style::default().fg(Color::DarkGray);
  for line in quote_lines {
    if line.spans.is_empty() {
      lines.push(Line::from(Span::styled(BORDER_VERTICAL.to_string(), style)));
      continue;
    }
    let mut spans = Vec::with_capacity(line.spans.len() + 1);
    spans.push(Span::styled(format!("{BORDER_VERTICAL} "), style));
    spans.extend(line.spans);
    lines.push(Line::from(spans));
  }
}

fn rule_line(width: usize) -> Line<'static> {
  return Line::from(Span::styled(
    BORDER_HORIZONTAL.repeat(width),
    Style::default().fg(Color::DarkGray),
  ));
}

const TABLE_MIN_COLUMN_WIDTH: usize = 3;

fn render_table(table: &Table, width: usize, lines: &mut Vec<Line<'static>>) {
  let columns = table
    .alignments
    .len()
    .max(table.header.len())
    .max(table.rows.iter().map(Vec::len).max().unwrap_or(0));
  if columns == 0 {
    return;
  }
  if width < 6 * columns + 1 {
    render_table_plain(table, width, lines);
    return;
  }
  let available = width - 3 * columns - 1;
  let widths = fit_widths(&natural_widths(table, columns), available);
  let border = Style::default().fg(Color::DarkGray);
  lines.push(table_border(
    BORDER_TOP_LEFT,
    BORDER_TOP_JUNCTION,
    BORDER_TOP_RIGHT,
    &widths,
    border,
  ));
  render_table_row(
    &table.header,
    &widths,
    &table.alignments,
    true,
    border,
    lines,
  );
  lines.push(table_border(
    BORDER_LEFT_JUNCTION,
    BORDER_CROSS,
    BORDER_RIGHT_JUNCTION,
    &widths,
    border,
  ));
  for row in &table.rows {
    render_table_row(row, &widths, &table.alignments, false, border, lines);
  }
  lines.push(table_border(
    BORDER_BOTTOM_LEFT,
    BORDER_BOTTOM_JUNCTION,
    BORDER_BOTTOM_RIGHT,
    &widths,
    border,
  ));
}

fn natural_widths(table: &Table, columns: usize) -> Vec<usize> {
  let mut widths = vec![TABLE_MIN_COLUMN_WIDTH; columns];
  for (index, cell) in table.header.iter().enumerate() {
    widths[index] = widths[index].max(cell_width(cell));
  }
  for row in &table.rows {
    for (index, cell) in row.iter().enumerate() {
      widths[index] = widths[index].max(cell_width(cell));
    }
  }
  return widths;
}

fn cell_width(cell: &[Inline]) -> usize {
  return cell.iter().map(inline_width).sum();
}

fn inline_width(inline: &Inline) -> usize {
  return match inline {
    Inline::Text(text) | Inline::Code(text) => wrap::width(text),
    Inline::Strong(inner)
    | Inline::Emphasis(inner)
    | Inline::Strikethrough(inner)
    | Inline::Link(inner) => inner.iter().map(inline_width).sum(),
    Inline::SoftBreak | Inline::HardBreak => 1,
  };
}

fn fit_widths(naturals: &[usize], available: usize) -> Vec<usize> {
  let mut widths = naturals.to_vec();
  loop {
    let total: usize = widths.iter().sum();
    if total <= available {
      break;
    }
    let Some(widest) = widths.iter().copied().max() else {
      break;
    };
    if widest <= TABLE_MIN_COLUMN_WIDTH {
      break;
    }
    let count = widths.iter().filter(|width| **width == widest).count();
    let next = widths
      .iter()
      .copied()
      .filter(|width| *width < widest)
      .max()
      .unwrap_or(TABLE_MIN_COLUMN_WIDTH)
      .max(TABLE_MIN_COLUMN_WIDTH);
    let excess = total - available;
    let step = (widest - next).min(excess.div_ceil(count));
    for width in widths.iter_mut().filter(|width| **width == widest) {
      *width -= step;
    }
  }
  return widths;
}

fn table_border(
  left: &str,
  middle: &str,
  right: &str,
  widths: &[usize],
  border: Style,
) -> Line<'static> {
  let mut text = String::from(left);
  for (index, width) in widths.iter().enumerate() {
    if index > 0 {
      text.push_str(middle);
    }
    text.push_str(&BORDER_HORIZONTAL.repeat(width + 2));
  }
  text.push_str(right);
  return Line::from(Span::styled(text, border));
}

fn render_table_row(
  cells: &[Vec<Inline>],
  widths: &[usize],
  alignments: &[Alignment],
  header: bool,
  border: Style,
  lines: &mut Vec<Line<'static>>,
) {
  let style = if header {
    Style::default().add_modifier(Modifier::BOLD)
  } else {
    Style::default()
  };
  let mut wrapped = Vec::with_capacity(widths.len());
  for (index, width) in widths.iter().enumerate() {
    let cell = cells.get(index).map_or(&[][..], Vec::as_slice);
    wrapped.push(wrap::wrap(segments(cell, style), *width));
  }
  let height = wrapped.iter().map(Vec::len).max().unwrap_or(1).max(1);
  for line_index in 0..height {
    let mut spans = vec![Span::styled(BORDER_VERTICAL, border)];
    for (index, width) in widths.iter().enumerate() {
      let alignment = alignments.get(index).copied().unwrap_or(Alignment::None);
      let content = wrapped[index].get(line_index);
      let content_width = content.map_or(0, |line| line.width());
      let padding = width.saturating_sub(content_width);
      let (left_padding, right_padding) = match alignment {
        Alignment::Right => (padding, 0),
        Alignment::Center => (padding / 2, padding - padding / 2),
        Alignment::Left | Alignment::None => (0, padding),
      };
      spans.push(Span::raw(" "));
      if left_padding > 0 {
        spans.push(Span::raw(" ".repeat(left_padding)));
      }
      if let Some(content) = content {
        spans.extend(content.spans.clone());
      }
      if right_padding > 0 {
        spans.push(Span::raw(" ".repeat(right_padding)));
      }
      spans.push(Span::raw(" "));
      spans.push(Span::styled(BORDER_VERTICAL, border));
    }
    lines.push(Line::from(spans));
  }
}

fn render_table_plain(
  table: &Table,
  width: usize,
  lines: &mut Vec<Line<'static>>,
) {
  let border = Style::default().fg(Color::DarkGray);
  for (row_index, row) in std::iter::once(&table.header)
    .chain(table.rows.iter())
    .enumerate()
  {
    let style = if row_index == 0 {
      Style::default().add_modifier(Modifier::BOLD)
    } else {
      Style::default()
    };
    let mut segments = Vec::new();
    for (index, cell) in row.iter().enumerate() {
      if index > 0 {
        segments.push((format!(" {BORDER_VERTICAL} "), border));
      }
      push_segments(cell, style, &mut segments);
    }
    lines.extend(wrap::wrap(segments, width));
  }
}

const CODE_BORDER_WIDTH: usize = 4;

fn render_code(
  lang: Option<&str>,
  text: &str,
  width: usize,
  lines: &mut Vec<Line<'static>>,
) {
  let border = Style::default().fg(Color::DarkGray);
  if width < CODE_BORDER_WIDTH + 2 {
    for logical in text.lines() {
      let segments = vec![(logical.to_string(), Style::default())];
      lines.extend(wrap::wrap(segments, width));
    }
    return;
  }
  let inner = width - CODE_BORDER_WIDTH;
  lines.push(code_top(width, border));
  if let Some(lang) = lang {
    let label = truncate_width(lang, inner);
    if !label.is_empty() {
      lines.push(code_label_line(&label, inner, border));
      lines.push(code_separator(width, border));
    }
  }
  for logical in text.lines() {
    for chunk in wrap_code(logical, inner) {
      lines.push(code_line(&chunk, inner, border));
    }
  }
  lines.push(code_bottom(width, border));
}

fn code_top(width: usize, border: Style) -> Line<'static> {
  return Line::from(Span::styled(
    format!(
      "{BORDER_TOP_LEFT}{}{BORDER_TOP_RIGHT}",
      BORDER_HORIZONTAL.repeat(width.saturating_sub(2))
    ),
    border,
  ));
}

fn code_bottom(width: usize, border: Style) -> Line<'static> {
  return Line::from(Span::styled(
    format!(
      "{BORDER_BOTTOM_LEFT}{}{BORDER_BOTTOM_RIGHT}",
      BORDER_HORIZONTAL.repeat(width.saturating_sub(2))
    ),
    border,
  ));
}

fn code_separator(width: usize, border: Style) -> Line<'static> {
  return Line::from(Span::styled(
    format!(
      "{BORDER_LEFT_JUNCTION}{}{BORDER_RIGHT_JUNCTION}",
      BORDER_HORIZONTAL.repeat(width.saturating_sub(2))
    ),
    border,
  ));
}

fn code_line(text: &str, inner: usize, border: Style) -> Line<'static> {
  let padding = inner.saturating_sub(wrap::width(text));
  return Line::from(vec![
    Span::styled(format!("{BORDER_VERTICAL} "), border),
    Span::raw(text.to_string()),
    Span::raw(" ".repeat(padding)),
    Span::styled(format!(" {BORDER_VERTICAL}"), border),
  ]);
}

fn code_label_line(label: &str, inner: usize, border: Style) -> Line<'static> {
  let padding = inner.saturating_sub(wrap::width(label));
  return Line::from(vec![
    Span::styled(format!("{BORDER_VERTICAL} "), border),
    Span::styled(label.to_string(), border.add_modifier(Modifier::BOLD)),
    Span::raw(" ".repeat(padding)),
    Span::styled(format!(" {BORDER_VERTICAL}"), border),
  ]);
}

fn wrap_code(line: &str, width: usize) -> Vec<String> {
  let line = line.replace('\t', "    ");
  if width == 0 || wrap::width(&line) <= width {
    return vec![line];
  }
  let mut chunks = Vec::new();
  let mut current = String::new();
  let mut current_width = 0;
  for ch in line.chars() {
    let char_width = wrap::char_width(ch);
    if current_width + char_width > width && current_width > 0 {
      chunks.push(std::mem::take(&mut current));
      current_width = 0;
    }
    current.push(ch);
    current_width += char_width;
  }
  if !current.is_empty() {
    chunks.push(current);
  }
  if chunks.is_empty() {
    chunks.push(String::new());
  }
  return chunks;
}

fn truncate_width(text: &str, width: usize) -> String {
  if wrap::width(text) <= width {
    return text.to_string();
  }
  let mut truncated = String::new();
  let mut used = 0;
  for ch in text.chars() {
    let char_width = wrap::char_width(ch);
    if used + char_width > width {
      break;
    }
    truncated.push(ch);
    used += char_width;
  }
  return truncated;
}

fn segments(inlines: &[Inline], style: Style) -> Vec<(String, Style)> {
  let mut segments = Vec::new();
  push_segments(inlines, style, &mut segments);
  return segments;
}

fn push_segments(
  inlines: &[Inline],
  style: Style,
  segments: &mut Vec<(String, Style)>,
) {
  for inline in inlines {
    match inline {
      Inline::Text(text) => segments.push((text.clone(), style)),
      Inline::Code(code) => {
        segments.push((code.clone(), style.fg(Color::Yellow)));
      }
      Inline::Strong(inner) => {
        push_segments(inner, style.add_modifier(Modifier::BOLD), segments);
      }
      Inline::Emphasis(inner) => {
        push_segments(inner, style.add_modifier(Modifier::ITALIC), segments);
      }
      Inline::Strikethrough(inner) => {
        push_segments(
          inner,
          style.add_modifier(Modifier::CROSSED_OUT),
          segments,
        );
      }
      Inline::Link(inner) => {
        push_segments(
          inner,
          style.fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
          segments,
        );
      }
      Inline::SoftBreak => segments.push((" ".to_string(), style)),
      Inline::HardBreak => segments.push(("\n".to_string(), style)),
    }
  }
}

fn heading_style(level: u8) -> Style {
  return Style::default()
    .fg(heading_color(level))
    .add_modifier(Modifier::BOLD);
}

fn heading_color(level: u8) -> Color {
  return match level {
    1 => Color::Cyan,
    2 => Color::LightBlue,
    3 => Color::Blue,
    4 => Color::Green,
    5 => Color::Yellow,
    _ => Color::Magenta,
  };
}

fn is_blank_line(line: &Line<'static>) -> bool {
  return line.spans.iter().all(|span| span.content.trim().is_empty());
}
