use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

use crate::session_manager::session::Session;

pub struct MessageLine {
  text: String,
  reasoning: bool,
}

impl MessageLine {
  pub fn new(text: String, reasoning: bool) -> Self {
    return Self { text, reasoning };
  }
}

pub struct MessagesList {
  messages: Vec<MessageLine>,
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
        let content = Line::from(Span::raw(m.text.clone()));
        let mut item = ListItem::new(content);
        if m.reasoning {
          item = item.style(Style::default().dim());
        }
        return item;
      })
      .collect();
    let messages = List::new(messages);
    frame.render_widget(messages, area);
  }

  pub fn add_message(&mut self, message: String) {
    self.messages.push(MessageLine::new(message, false));
  }

  pub fn ensure_reasoning_line(&mut self) {
    if self.messages.last().is_none_or(|last| !last.reasoning) {
      self
        .messages
        .push(MessageLine::new("Reasoning: ".to_string(), true));
    }
  }

  pub fn ensure_content_line(&mut self) {
    if self.messages.last().is_none_or(|last| last.reasoning) {
      self
        .messages
        .push(MessageLine::new("Fenrir: ".to_string(), false));
    }
  }

  pub fn append_to_last_message(&mut self, text: &str) {
    if let Some(last) = self.messages.last_mut() {
      last.text.push_str(text);
    } else {
      self
        .messages
        .push(MessageLine::new(format!("Fenrir: {}", text), false));
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
          "reasoning" => "Reasoning",
          _ => continue,
        };
        self.messages.push(MessageLine::new(
          format!("{}: {}", role, msg.content),
          msg.role == "reasoning",
        ));
      }
    }
  }
}
