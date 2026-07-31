use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{List, ListItem};

use crate::session_manager::session::Session;
use crate::tui::message_item::MessageItem;

pub struct MessagesList {
  messages: Vec<MessageItem>,
}

impl MessagesList {
  pub fn new() -> Self {
    return Self {
      messages: Vec::new(),
    };
  }

  pub fn draw(&self, frame: &mut Frame, area: Rect) {
    let messages: Vec<ListItem> =
      self.messages.iter().map(|m| m.to_list_item()).collect();
    let messages = List::new(messages);
    frame.render_widget(messages, area);
  }

  pub fn add_message(&mut self, message: String) {
    let message = MessageItem::new(message, false);
    self.messages.push(message);
  }

  pub fn ensure_reasoning_line(&mut self) {
    if self.messages.last().is_none_or(|last| !last.is_reasoning()) {
      let message = MessageItem::new("Reasoning: ".to_string(), true);
      self.messages.push(message);
    }
  }

  pub fn ensure_content_line(&mut self) {
    if self.messages.last().is_none_or(|last| last.is_reasoning()) {
      let message = MessageItem::new("Fenrir: ".to_string(), false);
      self.messages.push(message);
    }
  }

  pub fn append_to_last_message(&mut self, text: &str) {
    if let Some(last) = self.messages.last_mut() {
      last.push_string(text);
      return;
    }
    let message = MessageItem::new(format!("Fenrir: {}", text), false);
    self.messages.push(message);
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
  }
}
