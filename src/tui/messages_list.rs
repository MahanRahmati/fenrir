use ratatui::Frame;
use ratatui::crossterm::event::{MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::{Scrollbar, ScrollbarOrientation};
use tui_widget_list::{ListBuilder, ListState, ListView, ScrollDirection};

use crate::session_manager::session::Session;
use crate::tui::message_item::MessageItem;

pub struct MessagesList {
  messages: Vec<MessageItem>,
  list_state: ListState,
  follow: bool,
}

impl MessagesList {
  pub fn new() -> Self {
    return Self {
      messages: Vec::new(),
      list_state: ListState::default(),
      follow: true,
    };
  }

  pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
    let builder = ListBuilder::new(|context| {
      let message = &self.messages[context.index];
      message.to_list_item(context.cross_axis_size)
    });
    let list = ListView::new(builder, self.messages.len())
      .infinite_scrolling(false)
      .scroll_direction(ScrollDirection::Backward)
      .scrollbar(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
          .begin_symbol(None)
          .end_symbol(None),
      );
    frame.render_stateful_widget(list, area, &mut self.list_state);
  }

  fn select_last(&mut self) {
    if self.follow && !self.messages.is_empty() {
      self.list_state.select(Some(self.messages.len() - 1));
    }
  }

  pub fn handle_mouse(&mut self, mouse: &MouseEvent) {
    match mouse.kind {
      MouseEventKind::ScrollUp => self.scroll_older(),
      MouseEventKind::ScrollDown => self.scroll_newer(),
      _ => {}
    }
  }

  fn scroll_older(&mut self) {
    self.list_state.previous();
    self.follow = false;
  }

  fn scroll_newer(&mut self) {
    self.list_state.next();
    self.follow = !self.messages.is_empty()
      && self.list_state.selected == Some(self.messages.len() - 1);
  }

  pub fn add_message(&mut self, message: String) {
    let message = MessageItem::new(message, false);
    self.messages.push(message);
    self.select_last();
  }

  pub fn ensure_reasoning_line(&mut self) {
    if self.messages.last().is_none_or(|last| !last.is_reasoning()) {
      let message = MessageItem::new("Reasoning: ".to_string(), true);
      self.messages.push(message);
      self.select_last();
    }
  }

  pub fn ensure_content_line(&mut self) {
    if self.messages.last().is_none_or(|last| last.is_reasoning()) {
      let message = MessageItem::new("Fenrir: ".to_string(), false);
      self.messages.push(message);
      self.select_last();
    }
  }

  pub fn append_to_last_message(&mut self, text: &str) {
    if let Some(last) = self.messages.last_mut() {
      last.push_string(text);
    } else {
      let message = MessageItem::new(format!("Fenrir: {}", text), false);
      self.messages.push(message);
    }
    self.select_last();
  }

  pub fn update_messages(&mut self, session: Option<&Session>) {
    self.messages.clear();
    if let Some(session) = session {
      for msg in &session.messages {
        if msg.role == "system" {
          continue;
        }
        let role = match msg.role.as_str() {
          "user" => "You",
          "assistant" => "Fenrir",
          "reasoning" => "Reasoning",
          _ => continue,
        };
        let message = MessageItem::new(
          format!("{}: {}", role, msg.content),
          msg.role == "reasoning",
        );
        self.messages.push(message);
      }
    }
    self.select_last();
  }
}
