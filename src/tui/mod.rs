mod button;
mod completion_popup;
mod confirm_popup;
mod help_popup;
mod input;
mod message_item;
mod messages_list;
mod personas_popup;
mod popup;
mod popup_list;
mod sessions_popup;
mod statusline;

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Margin, Position, Rect};

use crate::persona_manager::Persona;
use crate::provider_manager::ChatUsage;
use crate::session_manager::session::Session;
use crate::slash_commands::SlashCommand;
use crate::tui::completion_popup::CompletionPopup;
use crate::tui::confirm_popup::ConfirmPopup;
use crate::tui::help_popup::HelpPopup;
use crate::tui::input::Input;
use crate::tui::messages_list::MessagesList;
use crate::tui::personas_popup::PersonasPopup;
use crate::tui::sessions_popup::SessionsPopup;
use crate::tui::statusline::StatusLine;

const TOP_PADDING: u16 = 1;
const HORIZONTAL_PADDING: u16 = 2;

pub struct Tui {
  input: Input,
  messages_list: MessagesList,
  running: bool,
  help_popup: HelpPopup,
  sessions_popup: SessionsPopup,
  personas_popup: PersonasPopup,
  confirm_popup: ConfirmPopup,
  completion_popup: CompletionPopup,
  usage: Option<ChatUsage>,
}

impl Tui {
  pub fn new() -> Self {
    return Self {
      input: Input::new(),
      messages_list: MessagesList::new(),
      running: true,
      help_popup: HelpPopup::new(),
      sessions_popup: SessionsPopup::new(),
      personas_popup: PersonasPopup::new(),
      confirm_popup: ConfirmPopup::new(),
      completion_popup: CompletionPopup::new(),
      usage: None,
    };
  }

  pub fn set_usage(&mut self, usage: Option<ChatUsage>) {
    self.usage = usage;
  }

  pub fn set_persona(&mut self, name: &str) {
    self.input.set_persona(name);
  }

  pub fn is_running(&self) -> bool {
    return self.running;
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let has_messages = !self.messages_list.is_empty();
    let (messages_area, input_area, status_bar_area) =
      self.compute_areas(frame, has_messages);

    if has_messages {
      self.messages_list.draw(frame, messages_area);
    }

    self.completion_popup.draw(frame, input_area);

    self.input.update_scroll_for_height(input_area.height);
    frame.render_widget(self.input.widget(input_area.height), input_area);

    let statusline = StatusLine::new(self.usage);
    frame
      .render_widget(statusline.draw(status_bar_area.width), status_bar_area);

    if !self.any_popup_open() {
      let (cursor_row, cursor_col) = self.input.cursor_position();
      frame.set_cursor_position(Position::new(
        input_area.x + cursor_col as u16 + 2,
        input_area.y + (cursor_row - self.input.scroll_offset) as u16 + 1,
      ));
    }

    self.sessions_popup.draw(frame);
    self.personas_popup.draw(frame);
    self.help_popup.draw(frame);
    self.confirm_popup.draw(frame);
  }

  fn compute_areas(
    &mut self,
    frame: &Frame,
    has_messages: bool,
  ) -> (Rect, Rect, Rect) {
    let screen_width = frame.area().width;
    let screen_height = frame.area().height;

    let box_width = if has_messages {
      screen_width.saturating_sub(HORIZONTAL_PADDING * 2)
    } else {
      popup::constrained_width(screen_width).saturating_sub(1)
    };
    self.input.set_width(box_width);

    let box_height = self.input.desired_box_height(if has_messages {
      screen_height.saturating_sub(4)
    } else {
      screen_height.saturating_sub(2)
    });

    let slot_height = if has_messages {
      box_height + TOP_PADDING * 2
    } else {
      box_height
    };

    let (messages_area, input_area, status_bar_area) = if has_messages {
      let [messages_area, input_area, status_bar_area] =
        frame.area().layout(&Layout::vertical([
          Constraint::Min(1),
          Constraint::Length(slot_height),
          Constraint::Length(1),
        ]));
      (messages_area, input_area, status_bar_area)
    } else {
      let [messages_area, input_area, _spacer, status_bar_area] =
        frame.area().layout(&Layout::vertical([
          Constraint::Min(1),
          Constraint::Length(slot_height),
          Constraint::Min(1),
          Constraint::Length(1),
        ]));
      (messages_area, input_area, status_bar_area)
    };

    let input_area = if has_messages {
      input_area.inner(Margin {
        horizontal: HORIZONTAL_PADDING,
        vertical: TOP_PADDING,
      })
    } else {
      let width = popup::constrained_width(input_area.width).saturating_sub(1);
      Rect::new(
        input_area.x + input_area.width.saturating_sub(width) / 2,
        input_area.y,
        width,
        input_area.height,
      )
    };
    return (messages_area, input_area, status_bar_area);
  }

  fn any_popup_open(&self) -> bool {
    return self.sessions_popup.is_open()
      || self.personas_popup.is_open()
      || self.help_popup.is_open()
      || self.confirm_popup.is_open()
      || self.completion_popup.is_open();
  }

  pub async fn read_input(&mut self) -> Option<String> {
    let event = match event::read() {
      Ok(e) => e,
      Err(e) => {
        eprintln!("TUI event error: {e}");
        return None;
      }
    };

    if let Event::Mouse(mouse) = &event {
      self.messages_list.handle_mouse(mouse);
    }

    if let Some(key) = event.as_key_press_event() {
      let is_shift_enter = key.code == KeyCode::Enter
        && key.modifiers.contains(KeyModifiers::SHIFT);

      if self.confirm_popup.handle_input(key.code) {
        return None;
      }
      if self.help_popup.handle_input(key.code) {
        return None;
      }
      if self.sessions_popup.handle_input(key.code) {
        return None;
      }
      if self.personas_popup.handle_input(key.code) {
        return None;
      }

      if is_shift_enter {
        self.input.enter_char('\n');
        self.completion_popup.refresh(&self.input).await;
        return None;
      }

      if self.completion_popup.is_open()
        && self
          .completion_popup
          .handle_input(key.code, &mut self.input)
          .await
      {
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
        KeyCode::Char(to_insert) => {
          self.input.enter_char(to_insert);
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::Backspace => {
          self.input.delete_char();
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::Left => {
          self.input.move_cursor_left();
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::Right => {
          self.input.move_cursor_right();
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::Up => {
          self.input.move_cursor_up();
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::Down => {
          self.input.move_cursor_down();
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::PageUp => {
          self.messages_list.scroll_page_older();
        }
        KeyCode::PageDown => {
          self.messages_list.scroll_page_newer();
        }
        _ => {}
      };
    }
    return None;
  }

  pub fn add_message(&mut self, message: String) {
    self.messages_list.add_message(message);
  }

  pub fn add_tool_call(&mut self, name: String, arguments: String) {
    self.messages_list.add_tool_call(name, arguments);
  }

  pub fn add_tool_result(&mut self, result: String, error: bool) {
    self.messages_list.add_tool_result(result, error);
  }

  pub fn ensure_reasoning_line(&mut self) {
    self.messages_list.ensure_reasoning_line();
  }

  pub fn ensure_content_line(&mut self) {
    self.messages_list.ensure_content_line();
  }

  pub fn append_to_last_message(&mut self, text: &str) {
    self.messages_list.append_to_last_message(text);
  }

  pub fn update_messages(&mut self, session: Option<&Session>) {
    self.messages_list.update_messages(session);
  }

  pub fn open_help_popup(&mut self, commands: Vec<SlashCommand>) {
    self.help_popup.open_help_popup(commands);
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

  pub fn open_personas_popup(
    &mut self,
    personas: &[Persona],
    current_persona: Option<&Persona>,
  ) {
    self
      .personas_popup
      .open_personas_popup(personas, current_persona);
  }

  pub fn get_selected_persona(&mut self) -> Option<usize> {
    return self.personas_popup.get_selected_persona();
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
