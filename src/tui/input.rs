pub struct Input {
  pub content: String,
  pub character_index: usize,
}

impl Input {
  pub fn new() -> Self {
    return Self {
      content: String::new(),
      character_index: 0,
    };
  }

  pub fn move_cursor_left(&mut self) {
    let cursor_moved_left = self.character_index.saturating_sub(1);
    self.character_index = self.clamp_cursor(cursor_moved_left);
  }

  pub fn move_cursor_right(&mut self) {
    let cursor_moved_right = self.character_index.saturating_add(1);
    self.character_index = self.clamp_cursor(cursor_moved_right);
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

  const fn reset_cursor(&mut self) {
    self.character_index = 0;
  }

  pub fn submit_content(&mut self) -> String {
    let value = std::mem::take(&mut self.content);
    self.reset_cursor();
    return value;
  }
}
