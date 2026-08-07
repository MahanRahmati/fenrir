use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::text::Line;
use ratatui::widgets::ListItem;

use crate::session_manager::session::Session;
use crate::tui::popup_list::PopupList;
use crate::utils::datetime::format_timestamp;

pub struct SessionsPopup {
  list: PopupList,
  sessions: Vec<Session>,
  current_session: Option<Session>,
}

impl SessionsPopup {
  pub fn new() -> Self {
    return Self {
      list: PopupList::new(),
      sessions: Vec::new(),
      current_session: None,
    };
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let sessions = &self.sessions;
    let current_session = &self.current_session;
    self.list.draw(frame, "Sessions", |popup_width| {
      sessions
        .iter()
        .map(|s| {
          let marker = match current_session {
            Some(session) if session.id == s.id => "*",
            _ => " ",
          };
          let label = format!(
            "{marker}{}",
            s.title.as_deref().unwrap_or("Untitled Session")
          );
          let timestamp_str = format_timestamp(s.last_interaction);
          let spacer_width = popup_width
            .saturating_sub(4)
            .saturating_sub(label.len() + timestamp_str.len());
          let spacer = " ".repeat(spacer_width);
          let content = Line::from(format!("{label}{spacer}{timestamp_str}"));
          ListItem::new(content)
        })
        .collect()
    });
  }

  pub fn handle_input(&mut self, key: KeyCode) -> bool {
    return self.list.handle_input(key);
  }

  pub fn open_sessions_popup(
    &mut self,
    sessions: Vec<Session>,
    current_session: Option<&Session>,
  ) {
    self.sessions = sessions;
    self.current_session = current_session.cloned();
    self.list.open(self.sessions.len());
  }

  pub fn get_selected_session(&mut self) -> Option<i64> {
    return self
      .list
      .take_selected()
      .and_then(|index| self.sessions.get(index))
      .map(|s| s.id);
  }

  pub fn is_open(&self) -> bool {
    return self.list.is_open();
  }
}
