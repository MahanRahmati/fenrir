use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

use crate::session_manager::session::Session;

pub struct MessagesList {
  messages: Vec<String>,
}

impl MessagesList {
  pub fn new() -> Self {
    return Self {
      messages: Vec::new(),
    };
  }

  pub fn draw(&self, frame: &mut Frame, area: Rect) {
    let messages: Vec<ListItem> = self
      .messages
      .iter()
      .map(|m| {
        let content = Line::from(Span::raw(m.clone()));
        ListItem::new(content)
      })
      .collect();
    let messages = List::new(messages);
    frame.render_widget(messages, area);
  }

  pub fn add_message(&mut self, message: String) {
    self.messages.push(message);
  }

  pub fn append_to_last_message(&mut self, text: &str) {
    if let Some(last) = self.messages.last_mut() {
      last.push_str(text);
    } else {
      self.messages.push(format!("Fenrir: {}", text));
    }
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
          _ => continue,
        };
        self.messages.push(format!("{}: {}", role, msg.content));
      }
    }
  }
}
