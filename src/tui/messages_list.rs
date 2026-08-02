use ratatui::Frame;
use ratatui::crossterm::event::{MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::{Scrollbar, ScrollbarOrientation};
use tui_widget_list::{ListBuilder, ListState, ListView, ScrollDirection};

use crate::session_manager::session::Session;
use crate::tui::message_item::{MessageItem, Role};

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
    let message = MessageItem::new(message, Role::User);
    self.messages.push(message);
    self.select_last();
  }

  pub fn add_tool_call(&mut self, name: String, arguments: String) {
    let message = MessageItem::new_tool_call(name, arguments);
    self.messages.push(message);
    self.select_last();
  }

  pub fn add_tool_result(&mut self, result: String, error: bool) {
    if let Some(last) = self.messages.last_mut()
      && last.is_pending_tool()
    {
      last.push_tool_result(result, error);
    } else {
      let message =
        MessageItem::new(result, Role::ToolResult).with_error(error);
      self.messages.push(message);
    }
    self.select_last();
  }

  pub fn ensure_reasoning_line(&mut self) {
    if self.messages.last().is_none_or(|last| !last.is_reasoning()) {
      let message = MessageItem::new(String::new(), Role::Reasoning);
      self.messages.push(message);
      self.select_last();
    }
  }

  pub fn ensure_content_line(&mut self) {
    if self.messages.last().is_none_or(|last| last.is_reasoning()) {
      let message = MessageItem::new(String::new(), Role::Assistant);
      self.messages.push(message);
      self.select_last();
    }
  }

  pub fn append_to_last_message(&mut self, text: &str) {
    if let Some(last) = self.messages.last_mut() {
      last.push_string(text);
    } else {
      let message = MessageItem::new(text.to_string(), Role::Assistant);
      self.messages.push(message);
    }
    self.select_last();
  }

  pub fn update_messages(&mut self, session: Option<&Session>) {
    self.messages.clear();
    if let Some(session) = session {
      let messages = &session.messages;
      let mut i = 0;
      while i < messages.len() {
        let msg = &messages[i];
        if msg.role == "system" {
          i += 1;
          continue;
        }
        if msg.role == "assistant" && msg.tool_calls.is_some() {
          let (name, arguments) = msg
            .tool_calls
            .as_ref()
            .and_then(|calls| calls.first())
            .map(|call| {
              (call.function.name.clone(), call.function.arguments.clone())
            })
            .unwrap_or_default();
          let mut item = MessageItem::new_tool_call(name, arguments);
          if let Some(call_id) = &msg.tool_call_id {
            for (j, result_msg) in messages.iter().enumerate().skip(i + 1) {
              if result_msg.role == "tool"
                && result_msg.tool_call_id.as_deref() == Some(call_id.as_str())
              {
                let content = result_msg
                  .content
                  .strip_prefix("Tool result: ")
                  .unwrap_or(&result_msg.content);
                let error = content.starts_with("Tool error: ");
                item.push_tool_result(content.to_string(), error);
                i = j;
                break;
              }
            }
          }
          self.messages.push(item);
          i += 1;
          continue;
        }
        if msg.role == "tool" {
          let content = msg
            .content
            .strip_prefix("Tool result: ")
            .unwrap_or(&msg.content);
          let error = content.starts_with("Tool error: ");
          let message = MessageItem::new(content.to_string(), Role::ToolResult)
            .with_error(error);
          self.messages.push(message);
          i += 1;
          continue;
        }
        let role = match msg.role.as_str() {
          "user" => Role::User,
          "assistant" => Role::Assistant,
          "reasoning" => Role::Reasoning,
          _ => {
            i += 1;
            continue;
          }
        };
        let message = MessageItem::new(msg.content.clone(), role);
        self.messages.push(message);
        i += 1;
      }
    }
    self.select_last();
  }
}
