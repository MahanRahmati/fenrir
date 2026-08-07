use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Constraint;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{
  List, ListItem, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState,
};

use crate::persona_manager::Persona;
use crate::tui::popup::{Popup, constrained_width};

pub struct PersonasPopup {
  popup: Popup,
  persona_index: usize,
  personas: Vec<Persona>,
  current_persona: Option<String>,
  selected_persona_index: Option<usize>,
  list_state: ListState,
}

impl PersonasPopup {
  pub fn new() -> Self {
    return Self {
      popup: Popup::new(),
      persona_index: 0,
      personas: Vec::new(),
      current_persona: None,
      selected_persona_index: None,
      list_state: ListState::default(),
    };
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let area = frame.area();
    let popup_area = area.centered(
      Constraint::Length(constrained_width(area.width)),
      Constraint::Percentage(30),
    );

    let popup_width = popup_area.width as usize;
    let personas: Vec<ListItem> = self
      .personas
      .iter()
      .map(|p| {
        let current_persona = match &self.current_persona {
          Some(name) => match name == p.name() {
            true => "*",
            false => " ",
          },
          None => " ",
        };
        let label = format!("{current_persona}{}", p.name());
        let label_width = label.len();
        let description = format!("- {}", p.description());
        let available_width = popup_width.saturating_sub(4);
        let spacer_width =
          available_width.saturating_sub(label_width + description.len());
        let spacer = " ".repeat(spacer_width);
        let content = Line::from(format!("{label}{spacer}{description}"));
        ListItem::new(content)
      })
      .collect();

    if let Some(list_area) =
      self.popup.draw_header(frame, "Personas", popup_area)
    {
      let list =
        List::new(personas).highlight_style(Style::default().reversed());
      frame.render_stateful_widget(list, list_area, &mut self.list_state);

      let mut scrollbar_state =
        ScrollbarState::new(self.personas.len()).position(self.persona_index);
      frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
          .begin_symbol(None)
          .end_symbol(None),
        list_area,
        &mut scrollbar_state,
      );
    }
  }

  pub fn handle_input(&mut self, key: KeyCode) -> bool {
    if !self.popup.is_open() {
      return false;
    }

    match key {
      KeyCode::Up => {
        if self.persona_index > 0 {
          self.persona_index -= 1;
          self.list_state.select(Some(self.persona_index));
        }
        return true;
      }
      KeyCode::Down => {
        if self.personas.is_empty() {
          return true;
        }
        if self.persona_index < self.personas.len() - 1 {
          self.persona_index += 1;
          self.list_state.select(Some(self.persona_index));
        }
        return true;
      }
      KeyCode::Enter => {
        self.selected_persona_index = Some(self.persona_index);
        self.close_personas_popup();
        return true;
      }
      KeyCode::Esc => {
        self.close_personas_popup();
        return true;
      }
      _ => {
        self.close_personas_popup();
        return true;
      }
    }
  }

  pub fn open_personas_popup(
    &mut self,
    personas: &'static [Persona],
    current_persona: Option<&Persona>,
  ) {
    self.personas = personas.to_vec();
    self.current_persona = current_persona.map(|p| p.name().to_string());
    self.persona_index = 0;
    self.list_state.select(Some(0));
    self.popup.open_popup();
  }

  fn close_personas_popup(&mut self) {
    self.popup.close_popup();
  }

  pub fn get_selected_persona(&mut self) -> Option<usize> {
    return self.selected_persona_index.take();
  }

  pub fn is_open(&self) -> bool {
    return self.popup.is_open();
  }
}
