mod empty;
mod session;

use ratatui::layout::{Constraint, Layout, Rect};

use crate::tui::input::Input;
use crate::tui::queued_messages_list::QueueMessagesList;

pub const SIDEBAR_WIDTH: u16 = 28;
pub const MIN_TERMINAL_WIDTH: u16 = 80;

pub struct Areas {
  pub sidebar: Option<Rect>,
  pub messages: Rect,
  pub queued_messages: Rect,
  pub input: Rect,
  pub status_bar: Rect,
}

pub struct ContentAreas {
  pub messages: Rect,
  pub queued_messages: Rect,
  pub input: Rect,
}

pub fn layout(
  area: Rect,
  input: &mut Input,
  queued_messages: &QueueMessagesList,
  has_messages: bool,
  has_queued_messages: bool,
  sidebar_visible: bool,
) -> Areas {
  let [body_area, status_bar] = area.layout(&Layout::vertical([
    Constraint::Min(1),
    Constraint::Length(1),
  ]));
  let (sidebar, content_area) =
    if sidebar_visible && body_area.width >= MIN_TERMINAL_WIDTH {
      let [sidebar, content] = body_area.layout(&Layout::horizontal([
        Constraint::Length(SIDEBAR_WIDTH),
        Constraint::Min(1),
      ]));
      (Some(sidebar), content)
    } else {
      (None, body_area)
    };
  let content = if has_messages {
    session::layout(content_area, input, queued_messages, has_queued_messages)
  } else {
    empty::layout(content_area, input)
  };
  return Areas {
    sidebar,
    messages: content.messages,
    queued_messages: content.queued_messages,
    input: content.input,
    status_bar,
  };
}
