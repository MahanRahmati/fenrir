use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

use crate::tui::popup::Popup;

pub struct ConfirmPopup {
  popup: Popup,
  message: String,
  selected: bool,
  confirmed: bool,
}

impl ConfirmPopup {
  pub fn new() -> Self {
    return Self {
      popup: Popup::new(),
      message: String::new(),
      selected: false,
      confirmed: false,
    };
  }

  pub fn draw(&self, frame: &mut Frame, messages_area: Rect) {
    if !self.popup.is_open() {
      return;
    }

    let message_lines: Vec<&str> = self.message.lines().collect();
    let popup_height = 5u16 + message_lines.len() as u16;
    let popup_y = messages_area.y + messages_area.height - popup_height;
    let popup_area =
      Rect::new(messages_area.x, popup_y, messages_area.width, popup_height);

    let confirm_btn = if !self.selected {
      "> Confirm"
    } else {
      "  Confirm"
    };
    let cancel_btn = if self.selected {
      "> Cancel"
    } else {
      "  Cancel"
    };
    let buttons = format!("    {}    {}", confirm_btn, cancel_btn);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    for line in &message_lines {
      lines.push(Line::from(*line));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(buttons));
    lines.push(Line::from(""));

    let paragraph =
      Paragraph::new(lines).block(Block::bordered().title("Confirm"));
    self.popup.draw(frame, paragraph, popup_area);
  }

  pub fn handle_input(&mut self, key: KeyCode) -> bool {
    if !self.popup.is_open() {
      return false;
    }

    match key {
      KeyCode::Left | KeyCode::Right => {
        self.selected = !self.selected;
        return true;
      }
      KeyCode::Enter => {
        self.confirmed = !self.selected;
        self.popup.close_popup();
        return true;
      }
      KeyCode::Esc => {
        self.popup.close_popup();
        return true;
      }
      _ => {
        return true;
      }
    }
  }

  pub fn open_confirm_popup(&mut self, message: String) {
    self.message = message;
    self.selected = false;
    self.confirmed = false;
    self.popup.open_popup();
  }

  pub fn get_confirmation(&mut self) -> bool {
    let confirmed = self.confirmed;
    self.confirmed = false;
    return confirmed;
  }

  pub fn is_open(&self) -> bool {
    return self.popup.is_open();
  }
}
