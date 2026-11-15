use ratatui::layout::{Constraint, Layout, Rect};

use crate::tui::input::Input;
use crate::tui::popup;
use crate::tui::view::ContentAreas;

pub fn layout(area: Rect, input: &mut Input) -> ContentAreas {
  let box_width = popup::constrained_width(area.width).saturating_sub(1);
  input.set_width(box_width);
  let box_height = input.desired_box_height(area.height.saturating_sub(2));
  let [messages, queued_messages, input_area, _spacer] =
    area.layout(&Layout::vertical([
      Constraint::Min(1),
      Constraint::Length(0),
      Constraint::Length(box_height),
      Constraint::Min(1),
    ]));
  let width = popup::constrained_width(input_area.width).saturating_sub(1);
  let input = Rect::new(
    input_area.x + input_area.width.saturating_sub(width) / 2,
    input_area.y,
    width,
    input_area.height,
  );
  return ContentAreas {
    messages,
    queued_messages,
    input,
  };
}
