use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

pub fn constrained_width(screen_width: u16) -> u16 {
  return screen_width.saturating_mul(50).saturating_div(100).min(80);
}

pub fn constrained_height(content_rows: u16, max_rows: u16) -> u16 {
  return content_rows.min(max_rows);
}

pub struct Popup {
  show_popup: bool,
}

impl Popup {
  pub fn new() -> Self {
    return Self { show_popup: false };
  }

  pub fn draw_header(
    &self,
    frame: &mut Frame,
    title: &str,
    popup_area: Rect,
  ) -> Option<Rect> {
    if !self.show_popup {
      return None;
    }
    frame.render_widget(Clear, popup_area);
    frame.render_widget(
      Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded),
      popup_area,
    );
    let separator = "─".repeat(popup_area.width.saturating_sub(2) as usize);
    let header = Paragraph::new(vec![
      Line::from(format!(" {title}")),
      Line::from(separator),
    ]);
    let header_area = Rect::new(
      popup_area.x + 1,
      popup_area.y + 1,
      popup_area.width.saturating_sub(2),
      2,
    );
    frame.render_widget(header, header_area);
    return Some(Rect::new(
      popup_area.x + 1,
      popup_area.y + 3,
      popup_area.width.saturating_sub(2),
      popup_area.height.saturating_sub(4),
    ));
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
