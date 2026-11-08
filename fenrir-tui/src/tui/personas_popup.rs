use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::text::Line;
use ratatui::widgets::ListItem;

use crate::tui::popup_list::PopupList;
use fenrir_core::engine::protocol::PersonaInfo;

pub struct PersonasPopup {
  list: PopupList,
  personas: Vec<PersonaInfo>,
  current_persona: Option<String>,
}

impl PersonasPopup {
  pub fn new() -> Self {
    return Self {
      list: PopupList::new(),
      personas: Vec::new(),
      current_persona: None,
    };
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let personas = &self.personas;
    let current_persona = &self.current_persona;
    self.list.draw(frame, "Personas", |popup_width, indices| {
      indices
        .iter()
        .map(|&index| {
          let p = &personas[index];
          let marker = match current_persona {
            Some(name) if name == &p.name => "*",
            _ => " ",
          };
          let label = format!("{marker}{}", p.name);
          let description = format!("- {}", p.description);
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

  pub fn open_personas_popup(
    &mut self,
    personas: &[PersonaInfo],
    current_persona: Option<&str>,
  ) {
    self.personas = personas.to_vec();
    self.current_persona = current_persona.map(str::to_string);
    let searchable = self
      .personas
      .iter()
      .map(|p| format!("{} {}", p.name, p.description))
      .collect();
    self.list.open(searchable);
  }

  pub fn get_selected_persona(&mut self) -> Option<String> {
    return self
      .list
      .take_selected()
      .and_then(|index| self.personas.get(index))
      .map(|p| p.name.clone());
  }

  pub fn is_open(&self) -> bool {
    return self.list.is_open();
  }
}
