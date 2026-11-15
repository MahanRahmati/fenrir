use std::cmp::Reverse;
use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::crossterm::event::{
  KeyCode, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Alignment, Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
  List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
  ScrollbarState,
};
use uuid::Uuid;

use crate::tui::sidebar::Sidebar;
use crate::tui::spinner::Spinner;
use fenrir_core::engine::protocol::{SessionStatus, SessionSummary};
use fenrir_core::utils::datetime::format_timestamp;

pub struct SessionsSidebar {
  sidebar: Sidebar,
  sessions: Vec<SessionSummary>,
  current: Option<Uuid>,
  selected: usize,
  list_state: ListState,
  focused: bool,
  area: Rect,
  list_area: Rect,
  spinner: Spinner,
}

impl SessionsSidebar {
  pub fn new() -> Self {
    return Self {
      sidebar: Sidebar::new(),
      sessions: Vec::new(),
      current: None,
      selected: 0,
      list_state: ListState::default(),
      focused: false,
      area: Rect::default(),
      list_area: Rect::default(),
      spinner: Spinner::new(),
    };
  }

  pub fn is_visible(&self) -> bool {
    return self.sidebar.is_visible();
  }

  pub fn is_shown(&self) -> bool {
    return self.sidebar.is_visible() && self.area.width > 0;
  }

  pub fn is_focused(&self) -> bool {
    return self.focused;
  }

  pub fn set_focused(&mut self, focused: bool) {
    self.focused = focused;
  }

  pub fn toggle(&mut self) {
    self.sidebar.toggle();
  }

  pub fn clear_area(&mut self) {
    self.area = Rect::default();
    self.list_area = Rect::default();
  }

  pub fn contains(&self, column: u16, row: u16) -> bool {
    return self.area.width > 0
      && self.area.contains(Position::new(column, row));
  }

  pub fn update_sessions(&mut self, sessions: Vec<SessionSummary>) {
    let selected_id = self.selected_id();
    self.sessions = sessions;
    self.sort();
    self.restore_selection(selected_id);
  }

  pub fn update_session_status(
    &mut self,
    session_id: Uuid,
    status: SessionStatus,
  ) {
    let Some(session) = self
      .sessions
      .iter_mut()
      .find(|session| session.id == session_id)
    else {
      return;
    };
    session.status = status;
  }

  pub fn sync_session(&mut self, summary: SessionSummary) {
    let id = summary.id;
    self.upsert(summary);
    self.current = Some(id);
    self.select_id(id);
  }

  pub fn update_title(&mut self, session_id: Uuid, title: String) {
    let Some(session) = self
      .sessions
      .iter_mut()
      .find(|session| session.id == session_id)
    else {
      return;
    };
    session.title = Some(title);
  }

  pub fn touch_session(&mut self, session_id: Uuid) {
    let selected_id = self.selected_id();
    let Ok(elapsed) = SystemTime::now().duration_since(UNIX_EPOCH) else {
      return;
    };
    let Some(session) = self
      .sessions
      .iter_mut()
      .find(|session| session.id == session_id)
    else {
      return;
    };
    session.last_interaction = elapsed.as_secs() as i64;
    self.sort();
    self.restore_selection(selected_id);
  }

  pub fn selected_session(&self) -> Option<Uuid> {
    return self.selected_id();
  }

  pub fn handle_key(&mut self, key: KeyCode) -> Option<Uuid> {
    let page = self.page_size();
    match key {
      KeyCode::Up => self.move_selected(-1),
      KeyCode::Down => self.move_selected(1),
      KeyCode::PageUp => self.move_selected(-(page as isize)),
      KeyCode::PageDown => self.move_selected(page as isize),
      KeyCode::Home => self.set_selected(0),
      KeyCode::End => self.set_selected(self.sessions.len().saturating_sub(1)),
      KeyCode::Enter => return self.selected_id(),
      _ => {}
    }
    return None;
  }

  pub fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Uuid> {
    if !self.contains(mouse.column, mouse.row) {
      return None;
    }
    match mouse.kind {
      MouseEventKind::ScrollUp => self.move_selected(-1),
      MouseEventKind::ScrollDown => self.move_selected(1),
      MouseEventKind::Down(MouseButton::Left) => {
        if !self
          .list_area
          .contains(Position::new(mouse.column, mouse.row))
        {
          return None;
        }
        let row = mouse.row.saturating_sub(self.list_area.y) as usize;
        let index = self.list_state.offset() + row / 2;
        if index >= self.sessions.len() {
          return None;
        }
        self.set_selected(index);
        return self.selected_id();
      }
      _ => {}
    }
    return None;
  }

  pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
    let Some(inner) = self.sidebar.draw(frame, area) else {
      self.clear_area();
      return;
    };
    self.area = area;
    self.list_area = inner;

    if self.sessions.is_empty() {
      self.list_state.select(None);
      if inner.height == 0 || inner.width == 0 {
        return;
      }
      let row = inner.y + inner.height / 2;
      let empty_area = Rect::new(inner.x, row, inner.width, 1);
      frame.render_widget(
        Paragraph::new("No sessions")
          .alignment(Alignment::Center)
          .style(Style::default().fg(Color::DarkGray)),
        empty_area,
      );
      return;
    }

    let width = inner.width as usize;
    let current = self.current;
    let working_icon = self.spinner.icon();
    let items: Vec<ListItem> = self
      .sessions
      .iter()
      .map(|session| {
        let is_current = current == Some(session.id);
        let title_style = if is_current {
          Style::default().fg(Color::Yellow)
        } else {
          Style::default()
        };
        let (indicator, indicator_style) = match session.status {
          SessionStatus::Idle => (" ", Style::default()),
          SessionStatus::Working => {
            (working_icon, Style::default().fg(Color::Cyan))
          }
          SessionStatus::AwaitingApproval => {
            ("●", Style::default().fg(Color::Yellow))
          }
          SessionStatus::Completed => ("●", Style::default().fg(Color::Green)),
        };
        let title = truncate(
          session.title.as_deref().unwrap_or("Untitled Session"),
          width.saturating_sub(2),
        );
        let timestamp = truncate(
          &format_timestamp(session.last_interaction),
          width.saturating_sub(2),
        );
        let title_line = Line::from(vec![
          Span::styled(indicator, indicator_style),
          Span::raw(" "),
          Span::styled(title, title_style),
        ]);
        let timestamp_line = Line::from(vec![
          Span::raw("  "),
          Span::styled(timestamp, Style::default().fg(Color::DarkGray)),
        ]);
        ListItem::new(vec![title_line, timestamp_line])
      })
      .collect();

    let highlight = if self.focused {
      Style::default().reversed()
    } else {
      Style::default()
    };
    let list = List::new(items).highlight_style(highlight);
    frame.render_stateful_widget(list, inner, &mut self.list_state);

    if self.sessions.len().saturating_mul(2) > inner.height as usize {
      let mut scrollbar_state =
        ScrollbarState::new(self.sessions.len()).position(self.selected);
      frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
          .begin_symbol(None)
          .end_symbol(None),
        inner,
        &mut scrollbar_state,
      );
    }
  }

  fn upsert(&mut self, summary: SessionSummary) {
    let selected_id = self.selected_id();
    match self
      .sessions
      .iter_mut()
      .find(|session| session.id == summary.id)
    {
      Some(session) => *session = summary,
      None => self.sessions.push(summary),
    }
    self.sort();
    self.restore_selection(selected_id);
  }

  fn sort(&mut self) {
    self
      .sessions
      .sort_by_key(|session| Reverse(session.last_interaction));
  }

  fn selected_id(&self) -> Option<Uuid> {
    return self.sessions.get(self.selected).map(|session| session.id);
  }

  fn select_id(&mut self, id: Uuid) {
    if let Some(index) =
      self.sessions.iter().position(|session| session.id == id)
    {
      self.set_selected(index);
    }
  }

  fn restore_selection(&mut self, id: Option<Uuid>) {
    if self.sessions.is_empty() {
      self.selected = 0;
      self.list_state.select(None);
      return;
    }
    let index = id
      .and_then(|id| self.position_of(id))
      .or_else(|| self.current.and_then(|id| self.position_of(id)))
      .unwrap_or(self.selected.min(self.sessions.len() - 1));
    self.set_selected(index);
  }

  fn position_of(&self, id: Uuid) -> Option<usize> {
    return self.sessions.iter().position(|session| session.id == id);
  }

  fn set_selected(&mut self, index: usize) {
    if self.sessions.is_empty() {
      self.selected = 0;
      self.list_state.select(None);
      return;
    }
    self.selected = index.min(self.sessions.len() - 1);
    self.list_state.select(Some(self.selected));
  }

  fn move_selected(&mut self, delta: isize) {
    if self.sessions.is_empty() {
      return;
    }
    let index = if delta < 0 {
      self.selected.saturating_sub(delta.unsigned_abs())
    } else {
      self
        .selected
        .saturating_add(delta as usize)
        .min(self.sessions.len() - 1)
    };
    self.set_selected(index);
  }

  fn page_size(&self) -> usize {
    return (self.list_area.height as usize / 2).max(1);
  }
}

fn truncate(text: &str, width: usize) -> String {
  let text_len = text.chars().count();
  if text_len <= width {
    return text.to_string();
  }
  if width <= 3 {
    return ".".repeat(width);
  }
  let mut truncated: String = text.chars().take(width - 3).collect();
  truncated.push('…');
  return truncated;
}
