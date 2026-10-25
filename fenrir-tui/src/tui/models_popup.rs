use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::text::Line;
use ratatui::widgets::ListItem;

use crate::tui::popup_list::PopupList;
use fenrir_core::provider_manager::Model;

pub struct ModelsPopup {
  list: PopupList,
  models: Vec<Model>,
  current: Option<Model>,
}

impl ModelsPopup {
  pub fn new() -> Self {
    return Self {
      list: PopupList::new(),
      models: Vec::new(),
      current: None,
    };
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let models = &self.models;
    let current = &self.current;
    self.list.draw(frame, "Models", |popup_width| {
      models
        .iter()
        .map(|m| {
          let marker = match current {
            Some(c) if c == m => "*",
            _ => " ",
          };
          let label = format!("{marker} {}", m.label());
          let description = m.provider.to_string();
          let spacer_width = popup_width
            .saturating_sub(4)
            .saturating_sub(label.len() + description.len());
          let spacer = " ".repeat(spacer_width);
          let content = Line::from(format!("{label}{spacer}{description}"));
          ListItem::new(content)
        })
        .collect()
    });
  }

  pub fn handle_input(&mut self, key: KeyCode) -> bool {
    return self.list.handle_input(key);
  }

  pub fn open_models_popup(
    &mut self,
    models: Vec<Model>,
    current: Option<Model>,
  ) {
    self.models = models;
    self.current = current;
    self.list.open(self.models.len());
  }

  pub fn get_selected_model(&mut self) -> Option<Model> {
    return self
      .list
      .take_selected()
      .and_then(|index| self.models.get(index))
      .cloned();
  }

  pub fn is_open(&self) -> bool {
    return self.list.is_open();
  }
}
