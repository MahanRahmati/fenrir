pub struct PopupSearch {
  searchable: Vec<String>,
  query: String,
  matches: Vec<usize>,
}

impl PopupSearch {
  pub fn new() -> Self {
    return Self {
      searchable: Vec::new(),
      query: String::new(),
      matches: Vec::new(),
    };
  }

  pub fn reset(&mut self, searchable: Vec<String>) {
    self.searchable = searchable
      .into_iter()
      .map(|text| text.to_lowercase())
      .collect();
    self.query.clear();
    self.refilter();
  }

  pub fn query(&self) -> &str {
    return &self.query;
  }

  pub fn matches(&self) -> &[usize] {
    return &self.matches;
  }

  pub fn insert_char(&mut self, character: char) {
    self.query.push(character);
    self.refilter();
  }

  pub fn backspace(&mut self) {
    self.query.pop();
    self.refilter();
  }

  fn refilter(&mut self) {
    let terms: Vec<String> = self
      .query
      .split_whitespace()
      .map(|term| term.to_lowercase())
      .collect();
    self.matches = self
      .searchable
      .iter()
      .enumerate()
      .filter(|(_, text)| terms.iter().all(|term| text.contains(term)))
      .map(|(index, _)| index)
      .collect();
  }
}
