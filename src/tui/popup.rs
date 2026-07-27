use ratatui::{Frame, layout::Rect, widgets::Widget};

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
      frame.render_widget(widget, popup_area);
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
