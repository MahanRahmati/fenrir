use crate::tui::input::Input;

pub struct PromptHistory {
  entries: Vec<String>,
  cursor: Option<usize>,
  fresh: bool,
}

impl PromptHistory {
  pub fn new() -> Self {
    return Self {
      entries: Vec::new(),
      cursor: None,
      fresh: false,
    };
  }

  pub fn replace(&mut self, entries: Vec<String>) {
    self.entries = entries;
    self.cursor = None;
    self.fresh = false;
  }

  pub fn push(&mut self, content: String) {
    if content.trim().is_empty() {
      return;
    }
    self.entries.push(content);
    self.cursor = None;
    self.fresh = false;
  }

  pub fn reset(&mut self) {
    self.cursor = None;
    self.fresh = false;
  }

  pub fn is_fresh(&self) -> bool {
    return self.fresh;
  }

  pub fn disarm(&mut self) {
    self.fresh = false;
  }

  pub fn previous(&mut self, input: &mut Input) -> bool {
    if self.entries.is_empty() {
      return false;
    }
    let index = match self.cursor {
      Some(0) => 0,
      Some(index) => index - 1,
      None => {
        if !input.content.is_empty() {
          return false;
        }
        self.entries.len() - 1
      }
    };
    self.cursor = Some(index);
    self.fresh = true;
    input.set_content(self.entries[index].clone());
    return true;
  }

  pub fn next(&mut self, input: &mut Input) -> bool {
    let Some(index) = self.cursor else {
      return false;
    };
    let next = index + 1;
    if next >= self.entries.len() {
      self.cursor = None;
      self.fresh = false;
      input.set_content(String::new());
    } else {
      self.cursor = Some(next);
      self.fresh = true;
      input.set_content(self.entries[next].clone());
    }
    return true;
  }
}
