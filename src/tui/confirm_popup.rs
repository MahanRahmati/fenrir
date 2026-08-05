use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Margin, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::HORIZONTAL_PADDING;
use crate::tui::popup::Popup;

fn button_style(active: bool) -> Style {
  return if active {
    Style::default().reversed()
  } else {
    Style::default()
  };
}

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

    let popup_height = 5u16;
    let popup_y = messages_area.y + messages_area.height - popup_height + 1;
    let popup_area =
      Rect::new(messages_area.x, popup_y, messages_area.width, popup_height)
        .inner(Margin {
          horizontal: HORIZONTAL_PADDING,
          vertical: 0,
        });

    let confirm_btn = Span::styled(" Confirm ", button_style(!self.selected));
    let cancel_btn = Span::styled(" Cancel ", button_style(self.selected));
    let buttons = Line::from(vec![confirm_btn, Span::raw("    "), cancel_btn]);

    let paragraph = Paragraph::new(buttons);
    if let Some(content_area) =
      self.popup.draw_header(frame, &self.message, popup_area)
    {
      frame.render_widget(paragraph, content_area);
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
