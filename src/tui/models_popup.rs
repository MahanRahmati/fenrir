use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::text::Line;
use ratatui::widgets::ListItem;

use crate::provider_manager::ModelInfo;
use crate::tui::popup_list::PopupList;

pub struct ModelsPopup {
  list: PopupList,
  models: Vec<ModelInfo>,
}

impl ModelsPopup {
  pub fn new() -> Self {
    return Self {
      list: PopupList::new(),
      models: Vec::new(),
    };
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let models = &self.models;
    self.list.draw(frame, "Models", |popup_width| {
      models
        .iter()
        .map(|m| {
          let marker = if m.active { "*" } else { " " };
          let label = match &m.model.alias {
            Some(alias) => format!("{marker} {alias}"),
            None => format!("{marker} {}", m.model.name),
          };
          let description = format!("{}", m.provider);
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

  pub fn open_models_popup(&mut self, models: Vec<ModelInfo>) {
    self.models = models;
    self.list.open(self.models.len());
  }

  pub fn is_open(&self) -> bool {
    return self.list.is_open();
  }
}
