use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, BorderType, Borders};

pub struct Sidebar {
  visible: bool,
}

impl Sidebar {
  pub fn new() -> Self {
    return Self { visible: true };
  }

  pub fn draw(&self, frame: &mut Frame, area: Rect) -> Option<Rect> {
    if !self.visible || area.width == 0 || area.height == 0 {
      return None;
    }
    let block = Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    return Some(inner);
  }

  pub fn toggle(&mut self) {
    self.visible = !self.visible;
  }

  pub fn is_visible(&self) -> bool {
    return self.visible;
  }
}
