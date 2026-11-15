use ratatui::layout::{Constraint, Layout, Margin, Rect};

use crate::tui::input::Input;
use crate::tui::queued_messages_list::QueueMessagesList;
use crate::tui::view::ContentAreas;
use crate::tui::{HORIZONTAL_PADDING, TOP_PADDING};

pub fn layout(
  area: Rect,
  input: &mut Input,
  queued_messages: &QueueMessagesList,
  has_queued_messages: bool,
) -> ContentAreas {
  let box_width = area.width.saturating_sub(HORIZONTAL_PADDING * 2);
  input.set_width(box_width);
  let box_height = input.desired_box_height(area.height.saturating_sub(4));
  let slot_height = box_height + TOP_PADDING;
  let queued_height = if has_queued_messages {
    queued_messages.desired_height(box_width)
  } else {
    0
  };

  let [messages, queued, input_area] = area.layout(&Layout::vertical([
    Constraint::Min(1),
    Constraint::Length(queued_height),
    Constraint::Length(slot_height),
  ]));

  let input_inner = input_area.inner(Margin {
    horizontal: HORIZONTAL_PADDING,
    vertical: 0,
  });
  let input_area = Rect::new(
    input_inner.x,
    input_inner.y + TOP_PADDING,
    input_inner.width,
    input_inner.height.saturating_sub(TOP_PADDING),
  );
  let queued_messages = if has_queued_messages {
    queued.inner(Margin {
      horizontal: HORIZONTAL_PADDING,
      vertical: 0,
    })
  } else {
    queued
  };

  return ContentAreas {
    messages,
    queued_messages,
    input: input_area,
  };
}
