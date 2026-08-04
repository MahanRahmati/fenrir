use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Clear, StatefulWidget, Widget};

pub struct Popup {
  show_popup: bool,
}

impl Popup {
  pub fn new() -> Self {
    return Self { show_popup: false };
  }

  pub fn draw<W: Widget>(
    &self,
    frame: &mut Frame,
    widget: W,
    popup_area: Rect,
  ) {
    if self.show_popup {
      frame.render_widget(Clear, popup_area);
      frame.render_widget(widget, popup_area);
    }
  }

  pub fn draw_stateful<W: StatefulWidget>(
    &self,
    frame: &mut Frame,
    widget: W,
    state: &mut W::State,
    popup_area: Rect,
  ) {
    if self.show_popup {
      frame.render_widget(Clear, popup_area);
      frame.render_stateful_widget(widget, popup_area, state);
    }
  }

  pub fn open_popup(&mut self) {
    self.show_popup = true;
  }

  pub fn close_popup(&mut self) {
    self.show_popup = false;
  }

  pub fn is_open(&self) -> bool {
    return self.show_popup;
  }
}
