use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Constraint;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{
  List, ListItem, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState,
};

use crate::session_manager::session::Session;
use crate::tui::popup::{Popup, constrained_width};
use crate::utils::datetime::format_timestamp;

pub struct SessionsPopup {
  popup: Popup,
  session_index: usize,
  sessions: Vec<Session>,
  current_session: Option<Session>,
  selected_session_id: Option<i64>,
  list_state: ListState,
}

impl SessionsPopup {
  pub fn new() -> Self {
    return Self {
      popup: Popup::new(),
      session_index: 0,
      sessions: Vec::new(),
      current_session: None,
      selected_session_id: None,
      list_state: ListState::default(),
    };
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let area = frame.area();
    let popup_area = area.centered(
      Constraint::Length(constrained_width(area.width)),
      Constraint::Percentage(20),
    );

    let popup_width = popup_area.width as usize;
    let sessions: Vec<ListItem> = self
      .sessions
      .iter()
      .map(|s| {
        let current_session = match &self.current_session {
          Some(session) => match session.id == s.id {
            true => "*",
            false => " ",
          },
          None => " ",
        };
        let id_str = s.id.to_string();
        let timestamp_str = format_timestamp(s.last_interaction);
        let label = format!("{current_session}{id_str}");
        let label_width = label.len();
        let available_width = popup_width.saturating_sub(4);
        let spacer_width =
          available_width.saturating_sub(label_width + timestamp_str.len());
        let spacer = " ".repeat(spacer_width);
        let content = Line::from(format!("{label}{spacer}{timestamp_str}"));
        ListItem::new(content)
      })
      .collect();

    if let Some(list_area) =
      self.popup.draw_header(frame, "Sessions", popup_area)
    {
      let list =
        List::new(sessions).highlight_style(Style::default().reversed());
      frame.render_stateful_widget(list, list_area, &mut self.list_state);

      let mut scrollbar_state =
        ScrollbarState::new(self.sessions.len()).position(self.session_index);
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
        if self.session_index > 0 {
          self.session_index -= 1;
          self.list_state.select(Some(self.session_index));
        }
        return true;
      }
      KeyCode::Down => {
        if self.sessions.is_empty() {
          return true;
        }
        if self.session_index < self.sessions.len() - 1 {
          self.session_index += 1;
          self.list_state.select(Some(self.session_index));
        }
        return true;
      }
      KeyCode::Enter => {
        self.selected_session_id = Some(self.sessions[self.session_index].id);
        self.close_sessions_popup();
        return true;
      }
      KeyCode::Esc => {
        self.close_sessions_popup();
        return true;
      }
      _ => {
        self.close_sessions_popup();
        return true;
      }
    }
  }

  pub fn open_sessions_popup(
    &mut self,
    sessions: Vec<Session>,
    current_session: Option<&Session>,
  ) {
    self.sessions = sessions;
    self.current_session = current_session.cloned();
    self.session_index = 0;
    self.list_state.select(Some(0));
    self.popup.open_popup();
  }

  fn close_sessions_popup(&mut self) {
    self.popup.close_popup();
  }

  pub fn get_selected_session(&mut self) -> Option<i64> {
    return self.selected_session_id.take();
  }

  pub fn is_open(&self) -> bool {
    return self.popup.is_open();
  }
}
