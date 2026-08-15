use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph};

pub const MAX_INPUT_HEIGHT: u16 = 8;

pub struct Input {
  pub content: String,
  pub character_index: usize,
  pub width: usize,
  pub scroll_offset: usize,
  persona_name: String,
}

impl Input {
  pub fn new() -> Self {
    return Self {
      content: String::new(),
      character_index: 0,
      width: 1,
      scroll_offset: 0,
      persona_name: String::new(),
    };
  }

  pub fn set_persona(&mut self, name: &str) {
    self.persona_name = name.to_string();
  }

  pub fn move_cursor_left(&mut self) {
    let cursor_moved_left = self.character_index.saturating_sub(1);
    self.character_index = self.clamp_cursor(cursor_moved_left);
  }

  pub fn move_cursor_right(&mut self) {
    let cursor_moved_right = self.character_index.saturating_add(1);
    self.character_index = self.clamp_cursor(cursor_moved_right);
  }

  pub fn move_cursor_up(&mut self) {
    let (row, col) = self.cursor_position();
    if row == 0 {
      self.character_index = 0;
      return;
    }
    let starts = self.row_starts();
    let target = row - 1;
    let end = starts[target + 1];
    self.character_index =
      self.clamp_cursor(starts[target] + col.min(end - starts[target]));
  }

  pub fn move_cursor_down(&mut self) {
    let (row, col) = self.cursor_position();
    let starts = self.row_starts();
    if row + 1 >= starts.len() {
      self.character_index = self.content.chars().count();
      return;
    }
    let target = row + 1;
    let end = starts
      .get(target + 1)
      .copied()
      .unwrap_or(self.content.chars().count());
    self.character_index =
      self.clamp_cursor(starts[target] + col.min(end - starts[target]));
  }

  pub fn enter_char(&mut self, new_char: char) {
    let index = self.byte_index();
    self.content.insert(index, new_char);
    self.move_cursor_right();
  }

  fn byte_index(&self) -> usize {
    self
      .content
      .char_indices()
      .map(|(i, _)| i)
      .nth(self.character_index)
      .unwrap_or(self.content.len())
  }

  pub fn delete_char(&mut self) {
    let is_not_cursor_leftmost = self.character_index != 0;
    if is_not_cursor_leftmost {
      let current_index = self.character_index;
      let from_left_to_current_index = current_index - 1;
      let before_char_to_delete =
        self.content.chars().take(from_left_to_current_index);
      let after_char_to_delete = self.content.chars().skip(current_index);
      self.content =
        before_char_to_delete.chain(after_char_to_delete).collect();
      self.move_cursor_left();
    }
  }

  fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
    new_cursor_pos.clamp(0, self.content.chars().count())
  }

  pub fn wrapped_line_count(&self) -> usize {
    let width = self.width.max(1);
    return self
      .content
      .split('\n')
      .map(|line| line.chars().count().div_ceil(width).max(1))
      .sum();
  }

  pub fn wrapped_lines(&self) -> Vec<String> {
    let width = self.width.max(1);
    let mut lines = Vec::new();
    for logical in self.content.split('\n') {
      let mut line = String::new();
      let mut count = 0;
      let mut any = false;
      for c in logical.chars() {
        any = true;
        line.push(c);
        count += 1;
        if count >= width {
          lines.push(line);
          line = String::new();
          count = 0;
        }
      }
      if !any {
        lines.push(String::new());
      } else if !line.is_empty() {
        lines.push(line);
      }
    }
    return lines;
  }

  fn row_starts(&self) -> Vec<usize> {
    let width = self.width.max(1);
    let segments: Vec<&str> = self.content.split('\n').collect();
    let mut starts = vec![0];
    let mut pos = 0;
    for (i, logical) in segments.iter().enumerate() {
      let chars = logical.chars().count();
      let rows = chars.div_ceil(width).max(1);
      for k in 1..rows {
        starts.push(pos + k * width);
      }
      if i + 1 < segments.len() {
        starts.push(pos + chars + 1);
      }
      pos += chars + 1;
    }
    return starts;
  }

  pub fn cursor_position(&self) -> (usize, usize) {
    let starts = self.row_starts();
    let mut row = 0;
    for (i, start) in starts.iter().enumerate() {
      if *start <= self.character_index {
        row = i;
      } else {
        break;
      }
    }
    let col = self.character_index - starts[row];
    return (row, col);
  }

  pub fn update_scroll(&mut self, visible_lines: usize) {
    if visible_lines == 0 {
      return;
    }
    let (cursor_row, _) = self.cursor_position();
    if cursor_row < self.scroll_offset {
      self.scroll_offset = cursor_row;
    } else if cursor_row >= self.scroll_offset + visible_lines {
      self.scroll_offset = cursor_row - visible_lines + 1;
    }
  }

  pub fn set_width(&mut self, box_width: u16) {
    self.width = (box_width.saturating_sub(4) as usize).max(1);
  }

  pub fn desired_box_height(&self, max_height: u16) -> u16 {
    let wrapped = self.wrapped_line_count();
    let visible = wrapped.min((MAX_INPUT_HEIGHT - 4) as usize);
    (4 + visible as u16).min(max_height).max(4)
  }

  pub fn update_scroll_for_height(&mut self, area_height: u16) {
    self.update_scroll(area_height.saturating_sub(4) as usize);
  }

  pub fn widget(&self, area_height: u16) -> Paragraph<'static> {
    let visible = area_height.saturating_sub(4) as usize;
    let wrapped = self.wrapped_lines();
    let start = self.scroll_offset;
    let end = (start + visible).min(wrapped.len());
    let mut lines: Vec<Line> = wrapped[start..end]
      .iter()
      .cloned()
      .map(Line::from)
      .collect();
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
      self.persona_name.clone(),
      Style::default().fg(Color::Yellow).bold(),
    )));
    Paragraph::new(lines).style(Style::default()).block(
      Block::bordered()
        .border_type(BorderType::Rounded)
        .padding(Padding::new(1, 1, 0, 0)),
    )
  }

  const fn reset_cursor(&mut self) {
    self.character_index = 0;
  }

  pub fn submit_content(&mut self) -> String {
    let mut value = std::mem::take(&mut self.content);
    while value.ends_with('\n') {
      value.pop();
    }
    self.reset_cursor();
    self.scroll_offset = 0;
    return value;
  }
}
