use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Constraint;
use ratatui::style::Style;
use ratatui::widgets::{
  List, ListItem, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState,
};

use crate::tui::popup::{Popup, constrained_width};

pub struct PopupList {
  popup: Popup,
  item_index: usize,
  item_count: usize,
  selected_index: Option<usize>,
  list_state: ListState,
}

impl PopupList {
  pub fn new() -> Self {
    return Self {
      popup: Popup::new(),
      item_index: 0,
      item_count: 0,
      selected_index: None,
      list_state: ListState::default(),
    };
  }

  pub fn draw<F>(&mut self, frame: &mut Frame, title: &str, build_items: F)
  where
    F: FnOnce(usize) -> Vec<ListItem<'static>>,
  {
    let area = frame.area();
    let popup_area = area.centered(
      Constraint::Length(constrained_width(area.width)),
      Constraint::Percentage(30),
    );

    if let Some(list_area) = self.popup.draw_header(frame, title, popup_area) {
      let items = build_items(popup_area.width as usize);
      let list = List::new(items).highlight_style(Style::default().reversed());
      frame.render_stateful_widget(list, list_area, &mut self.list_state);

      let mut scrollbar_state =
        ScrollbarState::new(self.item_count).position(self.item_index);
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
        if self.item_index > 0 {
          self.item_index -= 1;
          self.list_state.select(Some(self.item_index));
        }
        return true;
      }
      KeyCode::Down => {
        if self.item_count > 0 && self.item_index < self.item_count - 1 {
          self.item_index += 1;
          self.list_state.select(Some(self.item_index));
        }
        return true;
      }
      KeyCode::Enter => {
        if self.item_count > 0 {
          self.selected_index = Some(self.item_index);
        }
        self.close();
        return true;
      }
      KeyCode::Esc => {
        self.close();
        return true;
      }
      _ => {
        self.close();
        return true;
      }
    }
  }

  pub fn open(&mut self, item_count: usize) {
    self.item_count = item_count;
    self.item_index = 0;
    self.list_state.select(Some(0));
    self.popup.open_popup();
  }

  pub fn close(&mut self) {
    self.popup.close_popup();
  }

  pub fn take_selected(&mut self) -> Option<usize> {
    return self.selected_index.take();
  }

  pub fn is_open(&self) -> bool {
    return self.popup.is_open();
  }
}
