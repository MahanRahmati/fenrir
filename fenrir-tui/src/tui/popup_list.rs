use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Constraint;
use ratatui::style::{Color, Style};
use ratatui::widgets::{
  List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
  ScrollbarState,
};

use crate::tui::popup::{Popup, constrained_height, constrained_width};
use crate::tui::popup_search::PopupSearch;

const MAX_VISIBLE_ITEMS: u16 = 10;

pub struct PopupList {
  popup: Popup,
  search: PopupSearch,
  selected: usize,
  selected_index: Option<usize>,
  list_state: ListState,
}

impl PopupList {
  pub fn new() -> Self {
    return Self {
      popup: Popup::new(),
      search: PopupSearch::new(),
      selected: 0,
      selected_index: None,
      list_state: ListState::default(),
    };
  }

  pub fn draw<F>(&mut self, frame: &mut Frame, title: &str, build_items: F)
  where
    F: FnOnce(usize, &[usize]) -> Vec<ListItem<'static>>,
  {
    let matches = self.search.matches();
    let item_count = matches.len();
    let area = frame.area();
    let popup_height =
      constrained_height(item_count.max(1) as u16, MAX_VISIBLE_ITEMS) + 6;
    let popup_area = area.centered(
      Constraint::Length(constrained_width(area.width)),
      Constraint::Length(popup_height.min(area.height)),
    );

    let query = self.search.query();
    if let Some(list_area) =
      self
        .popup
        .draw_header(frame, title, Some(query), popup_area)
    {
      if item_count == 0 {
        frame.render_widget(
          Paragraph::new(" No matches")
            .style(Style::default().fg(Color::DarkGray)),
          list_area,
        );
        return;
      }

      let items = build_items(popup_area.width as usize, matches);
      let list = List::new(items).highlight_style(Style::default().reversed());
      frame.render_stateful_widget(list, list_area, &mut self.list_state);

      if item_count > MAX_VISIBLE_ITEMS as usize {
        let mut scrollbar_state =
          ScrollbarState::new(item_count).position(self.selected);
        frame.render_stateful_widget(
          Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None),
          list_area,
          &mut scrollbar_state,
        );
      }
    }
  }

  pub fn handle_input(&mut self, key: KeyCode) -> bool {
    if !self.popup.is_open() {
      return false;
    }

    match key {
      KeyCode::Up => {
        if self.selected > 0 {
          self.selected -= 1;
          self.list_state.select(Some(self.selected));
        }
        return true;
      }
      KeyCode::Down => {
        if self.selected + 1 < self.search.matches().len() {
          self.selected += 1;
          self.list_state.select(Some(self.selected));
        }
        return true;
      }
      KeyCode::Enter => {
        self.selected_index = self.search.matches().get(self.selected).copied();
        self.close();
        return true;
      }
      KeyCode::Esc => {
        self.close();
        return true;
      }
      KeyCode::Backspace => {
        self.search.backspace();
        self.reset_selection();
        return true;
      }
      KeyCode::Char(character) => {
        self.search.insert_char(character);
        self.reset_selection();
        return true;
      }
      _ => {
        self.close();
        return true;
      }
    }
  }

  fn reset_selection(&mut self) {
    self.selected = 0;
    self.list_state.select(if self.search.matches().is_empty() {
      None
    } else {
      Some(0)
    });
    *self.list_state.offset_mut() = 0;
  }

  pub fn open(&mut self, searchable: Vec<String>) {
    self.search.reset(searchable);
    self.selected_index = None;
    self.reset_selection();
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
