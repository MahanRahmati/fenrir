use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Rect};

use crate::tui::button::Button;
use crate::tui::popup::{Popup, constrained_width};

const BUTTON_HEIGHT: u16 = 3;
const BUTTON_GAP: u16 = 4;

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

  pub fn draw(&self, frame: &mut Frame) {
    if !self.popup.is_open() {
      return;
    }

    let popup_height = 4u16 + BUTTON_HEIGHT;
    let popup_area = frame.area().centered(
      Constraint::Length(constrained_width(frame.area().width)),
      Constraint::Length(popup_height),
    );

    if let Some(content_area) =
      self.popup.draw_header(frame, &self.message, popup_area)
    {
      let confirm = Button::new("Confirm").selected(!self.selected);
      let cancel = Button::new("Cancel").selected(self.selected);
      let buttons_width = confirm.width() + BUTTON_GAP + cancel.width();
      let buttons_x =
        content_area.x + content_area.width.saturating_sub(buttons_width) / 2;
      let confirm_area = Rect::new(
        buttons_x,
        content_area.y + content_area.height - BUTTON_HEIGHT,
        confirm.width(),
        BUTTON_HEIGHT,
      );
      let cancel_area = Rect::new(
        confirm_area.x + confirm_area.width + BUTTON_GAP,
        confirm_area.y,
        cancel.width(),
        BUTTON_HEIGHT,
      );
      frame.render_widget(confirm, confirm_area);
      frame.render_widget(cancel, cancel_area);
    }
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
