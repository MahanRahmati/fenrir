mod confirm_popup;
mod help_popup;
mod input;
mod messages_list;
mod popup;
mod sessions_popup;
mod statusline;

use ratatui::Frame;
use ratatui::crossterm::event::{self, KeyCode};
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};

use crate::session_manager::session::Session;
use crate::tui::confirm_popup::ConfirmPopup;
use crate::tui::help_popup::HelpPopup;
use crate::tui::input::Input;
use crate::tui::messages_list::MessagesList;
use crate::tui::sessions_popup::SessionsPopup;
use crate::tui::statusline::StatusLine;

pub struct Tui {
  input: Input,
  messages_list: MessagesList,
  running: bool,
  help_popup: HelpPopup,
  sessions_popup: SessionsPopup,
  confirm_popup: ConfirmPopup,
}

impl Tui {
  pub fn new() -> Self {
    return Self {
      input: Input::new(),
      messages_list: MessagesList::new(),
      running: true,
      help_popup: HelpPopup::new(),
      sessions_popup: SessionsPopup::new(),
      confirm_popup: ConfirmPopup::new(),
    };
  }

  pub fn is_running(&self) -> bool {
    return self.running;
  }

  pub fn draw(&self, frame: &mut Frame) {
    let layout = Layout::vertical([
      Constraint::Min(1),
      Constraint::Length(3),
      Constraint::Length(1),
    ]);
    let [messages_area, input_area, status_bar_area] =
      frame.area().layout(&layout);

    self.messages_list.draw(frame, messages_area);

    let input = Paragraph::new(self.input.content.as_str())
      .style(Style::default())
      .block(Block::bordered());
    frame.render_widget(input, input_area);

    let statusline = StatusLine::new();
    frame
      .render_widget(statusline.draw(status_bar_area.width), status_bar_area);

    frame.set_cursor_position(Position::new(
      input_area.x + self.input.character_index as u16 + 1,
      input_area.y + 1,
    ));

    self.sessions_popup.draw(frame);
    self.help_popup.draw(frame, messages_area);
    self.confirm_popup.draw(frame, messages_area);
  }

  pub async fn read_input(&mut self) -> Option<String> {
    let event = match event::read() {
      Ok(e) => e,
      Err(e) => {
        eprintln!("TUI event error: {}", e);
        return None;
      }
    };

    if let Some(key) = event.as_key_press_event() {
      if self.confirm_popup.handle_input(key.code) {
        return None;
      }
      self.help_popup.handle_input();
      if self.sessions_popup.handle_input(key.code) {
        return None;
      }

      match key.code {
        KeyCode::Enter => {
          let input = self.input.submit_content();
          if input.is_empty() {
            return None;
          }
          return Some(input);
        }
        KeyCode::Char(to_insert) => self.input.enter_char(to_insert),
        KeyCode::Backspace => self.input.delete_char(),
        KeyCode::Left => self.input.move_cursor_left(),
        KeyCode::Right => self.input.move_cursor_right(),
        _ => {}
      };
    }
    return None;
  }

  pub fn add_message(&mut self, message: String) {
    self.messages_list.add_message(message);
  }

  pub fn append_to_last_message(&mut self, text: &str) {
    self.messages_list.append_to_last_message(text);
  }

  pub fn update_messages(&mut self, session: Option<&Session>) {
    self.messages_list.update_messages(session);
  }

  pub fn open_help_popup(&mut self) {
    self.help_popup.open_help_popup();
  }

  pub fn open_sessions_popup(
    &mut self,
    sessions: Vec<Session>,
    current_session: Option<&Session>,
  ) {
    self
      .sessions_popup
      .open_sessions_popup(sessions, current_session);
  }

  pub fn get_selected_session(&mut self) -> Option<i64> {
    return self.sessions_popup.get_selected_session();
  }

  pub fn open_confirm_popup(&mut self, message: String) {
    self.confirm_popup.open_confirm_popup(message);
  }

  pub fn get_confirmation(&mut self) -> bool {
    return self.confirm_popup.get_confirmation();
  }

  pub fn confirm_popup_open(&self) -> bool {
    return self.confirm_popup.is_open();
  }

  pub fn handle_confirm_event(&mut self, key: KeyCode) -> bool {
    return self.confirm_popup.handle_input(key);
  }

  pub fn quit(&mut self) {
    self.running = false;
  }
}
