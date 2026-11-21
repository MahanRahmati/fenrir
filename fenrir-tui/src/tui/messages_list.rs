use std::collections::{HashMap, HashSet};

use ratatui::Frame;
use ratatui::crossterm::event::{MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::{
  List, ListItem, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState,
};

use crate::tui::message_item::{MessageItem, Role};
use fenrir_core::session_manager::Message;
use fenrir_core::tool_manager::{
  ToolPreview, runs_subagent, tool::parse_tool_params,
};
use fenrir_core::utils::duration::{elapsed_seconds, finished_in, working_for};

struct WorkingTimer {
  started_at: i64,
}

pub struct MessagesList {
  messages: Vec<MessageItem>,
  list_state: ListState,
  follow: bool,
  viewport_height: u16,
  line_count: usize,
  subagent_calls: HashSet<String>,
  subagent_streams: HashMap<String, usize>,
  working: Option<WorkingTimer>,
}

impl MessagesList {
  pub fn new() -> Self {
    return Self {
      messages: Vec::new(),
      list_state: ListState::default(),
      follow: true,
      viewport_height: 0,
      line_count: 0,
      subagent_calls: HashSet::new(),
      subagent_streams: HashMap::new(),
      working: None,
    };
  }

  pub fn is_empty(&self) -> bool {
    return self.messages.is_empty();
  }

  pub fn start_working(&mut self, started_at: i64) {
    self.working = Some(WorkingTimer { started_at });
  }

  pub fn finish_working(&mut self, duration_seconds: u64) {
    self.working = None;
    let message =
      MessageItem::new(finished_in(duration_seconds), Role::Duration);
    self.messages.push(message);
    self.select_last();
  }

  pub fn clear_working(&mut self) {
    self.working = None;
  }

  pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
    self.viewport_height = area.height;
    let mut items: Vec<ListItem> = Vec::new();
    for message in &mut self.messages {
      items.extend(
        message
          .lines(area.width)
          .iter()
          .map(|line| ListItem::new(line.clone())),
      );
    }
    if let Some(working) = &self.working {
      let text = working_for(elapsed_seconds(working.started_at));
      let mut message = MessageItem::new(text, Role::Duration);
      items.extend(
        message
          .lines(area.width)
          .iter()
          .map(|line| ListItem::new(line.clone())),
      );
    }
    self.line_count = items.len();
    if self.follow {
      self.set_offset(self.max_offset());
      if self.line_count > 0 {
        self.list_state.select(Some(self.line_count - 1));
      } else {
        self.list_state.select(None);
      }
    } else {
      let offset = self.list_state.offset().min(self.max_offset());
      self.set_offset(offset);
      self.follow = offset >= self.max_offset();
    }
    let list_height = self.line_count.min(area.height as usize) as u16;
    let list_area = Rect::new(
      area.x,
      area.y + area.height.saturating_sub(list_height),
      area.width,
      list_height,
    );
    frame.render_stateful_widget(
      List::new(items),
      list_area,
      &mut self.list_state,
    );
    let viewport_height = area.height as usize;
    if self.line_count > viewport_height {
      let scrollbar_length = self.line_count - viewport_height + 1;
      let mut scrollbar_state = ScrollbarState::new(scrollbar_length)
        .viewport_content_length(viewport_height)
        .position(self.list_state.offset());
      frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
          .begin_symbol(None)
          .end_symbol(None),
        area,
        &mut scrollbar_state,
      );
    }
  }

  fn select_last(&mut self) {
    if self.follow && !self.messages.is_empty() {
      self.list_state.select_last();
    }
  }

  pub fn handle_mouse(&mut self, mouse: &MouseEvent) {
    match mouse.kind {
      MouseEventKind::ScrollUp => self.scroll_older(),
      MouseEventKind::ScrollDown => self.scroll_newer(),
      _ => {}
    }
  }

  fn max_offset(&self) -> usize {
    return self
      .line_count
      .saturating_sub(self.viewport_height as usize);
  }

  fn set_offset(&mut self, offset: usize) {
    *self.list_state.offset_mut() = offset;
    self.list_state.select(Some(offset));
  }

  fn scroll_older(&mut self) {
    let offset = self.list_state.offset().saturating_sub(1);
    self.set_offset(offset);
    self.follow = false;
  }

  fn scroll_newer(&mut self) {
    let max_offset = self.max_offset();
    let offset = (self.list_state.offset() + 1).min(max_offset);
    self.set_offset(offset);
    self.follow = offset >= max_offset;
  }

  pub fn scroll_page_older(&mut self) {
    let amount = (self.viewport_height / 2).max(1) as usize;
    let offset = self.list_state.offset().saturating_sub(amount);
    self.set_offset(offset);
    self.follow = false;
  }

  pub fn scroll_page_newer(&mut self) {
    let amount = (self.viewport_height / 2).max(1) as usize;
    let max_offset = self.max_offset();
    let offset = (self.list_state.offset() + amount).min(max_offset);
    self.set_offset(offset);
    self.follow = offset >= max_offset;
  }

  pub fn add_message(&mut self, message: String) {
    let message = MessageItem::new(message, Role::User);
    self.messages.push(message);
    self.select_last();
  }

  pub fn add_error(&mut self, message: String) {
    let message = MessageItem::new(message, Role::Error);
    self.messages.push(message);
    self.select_last();
  }

  pub fn add_tool_call(
    &mut self,
    id: String,
    name: String,
    arguments: String,
    preview: Option<ToolPreview>,
  ) {
    let is_subagent = runs_subagent(&name);
    let task = if is_subagent {
      subagent_task(&arguments)
    } else {
      None
    };
    if is_subagent {
      self.subagent_calls.insert(id.clone());
    }
    let message = MessageItem::new_tool_call(id, name, arguments, preview);
    self.messages.push(message);
    if let Some(task) = task {
      self.add_subagent_task(&task);
    }
    self.select_last();
  }

  pub fn add_tool_result(&mut self, id: String, result: String, error: bool) {
    if self.subagent_calls.remove(&id) {
      self.add_subagent_report(&id, result, error);
      return;
    }
    self.subagent_streams.remove(&id);
    if let Some(call) =
      self.messages.iter_mut().rev().find(|message| {
        message.depth() == 0 && message.is_pending_tool_call(&id)
      })
    {
      call.push_tool_result(result, error);
    } else {
      let message =
        MessageItem::new(result, Role::ToolResult).with_error(error);
      self.messages.push(message);
    }
    self.select_last();
  }

  pub fn add_subagent_token(
    &mut self,
    tool_call_id: &str,
    text: &str,
    reasoning: bool,
  ) {
    let role = if reasoning {
      Role::Reasoning
    } else {
      Role::Assistant
    };
    let index =
      self
        .subagent_streams
        .get(tool_call_id)
        .copied()
        .filter(|index| {
          self.messages.get(*index).is_some_and(|message| {
            message.depth() == 1
              && if reasoning {
                message.is_reasoning()
              } else {
                message.is_assistant()
              }
          })
        });
    match index {
      Some(index) => {
        if let Some(message) = self.messages.get_mut(index) {
          message.push_string(text);
        }
      }
      None => {
        let message = MessageItem::new(String::new(), role).with_depth(1);
        self.messages.push(message);
        let index = self.messages.len() - 1;
        self
          .subagent_streams
          .insert(tool_call_id.to_string(), index);
        if let Some(message) = self.messages.get_mut(index) {
          message.push_string(text);
        }
      }
    }
    self.select_last();
  }

  pub fn add_subagent_tool_call(
    &mut self,
    tool_call_id: &str,
    id: String,
    name: String,
    arguments: String,
    preview: Option<ToolPreview>,
  ) {
    self.subagent_streams.remove(tool_call_id);
    let message =
      MessageItem::new_tool_call(id, name, arguments, preview).with_depth(1);
    self.messages.push(message);
    self.select_last();
  }

  pub fn add_subagent_tool_result(
    &mut self,
    tool_call_id: &str,
    id: String,
    result: String,
    error: bool,
  ) {
    self.subagent_streams.remove(tool_call_id);
    if let Some(call) =
      self.messages.iter_mut().rev().find(|message| {
        message.depth() == 1 && message.is_pending_tool_call(&id)
      })
    {
      call.push_tool_result(result, error);
    } else {
      let message = MessageItem::new(result, Role::ToolResult)
        .with_error(error)
        .with_depth(1);
      self.messages.push(message);
    }
    self.select_last();
  }

  pub fn add_subagent_retry(
    &mut self,
    tool_call_id: &str,
    attempt: u32,
    max_attempts: u32,
    delay_seconds: u64,
  ) {
    self.subagent_streams.remove(tool_call_id);
    let message = MessageItem::new(
      format!(
        "Retrying in {delay_seconds}s (attempt {attempt}/{max_attempts})"
      ),
      Role::Reasoning,
    )
    .with_depth(1);
    self.messages.push(message);
    self.select_last();
  }

  pub fn add_subagent_report(
    &mut self,
    tool_call_id: &str,
    result: String,
    error: bool,
  ) {
    self.subagent_streams.remove(tool_call_id);
    let report = result.trim();
    let duplicate = !report.is_empty()
      && self.messages.last().is_some_and(|last| {
        last.depth() == 1 && last.is_assistant() && last.text().trim() == report
      });
    if !report.is_empty() && !duplicate {
      let message = MessageItem::new(result, Role::SubagentReport)
        .with_error(error)
        .with_depth(1);
      self.messages.push(message);
    }
  }

  fn add_subagent_task(&mut self, task: &str) {
    let task = task.trim();
    if task.is_empty() {
      return;
    }
    let message =
      MessageItem::new(task.to_string(), Role::SubagentTask).with_depth(1);
    self.messages.push(message);
  }

  pub fn add_summary(&mut self, summary: String) {
    let message = MessageItem::new(summary, Role::Compaction);
    self.messages.push(message);
    self.select_last();
  }

  pub fn ensure_reasoning_line(&mut self) {
    if self
      .messages
      .last()
      .is_none_or(|last| last.depth() != 0 || !last.is_reasoning())
    {
      let message = MessageItem::new(String::new(), Role::Reasoning);
      self.messages.push(message);
      self.select_last();
    }
  }

  pub fn ensure_content_line(&mut self) {
    if self
      .messages
      .last()
      .is_none_or(|last| last.depth() != 0 || !last.is_assistant())
    {
      let message = MessageItem::new(String::new(), Role::Assistant);
      self.messages.push(message);
      self.select_last();
    }
  }

  pub fn append_to_last_message(&mut self, text: &str) {
    if let Some(last) = self.messages.last_mut()
      && last.depth() == 0
    {
      last.push_string(text);
    } else {
      let message = MessageItem::new(text.to_string(), Role::Assistant);
      self.messages.push(message);
    }
    self.select_last();
  }

  pub fn update_messages(&mut self, messages: Option<&[Message]>) {
    self.messages.clear();
    self.subagent_calls.clear();
    self.subagent_streams.clear();
    self.working = None;
    if let Some(messages) = messages {
      let mut i = 0;
      while i < messages.len() {
        let msg = &messages[i];
        if msg.role == "system" {
          i += 1;
          continue;
        }
        if msg.role == "assistant" && msg.tool_calls.is_some() {
          let (id, name, arguments, preview) = msg
            .tool_calls
            .as_ref()
            .and_then(|calls| calls.first())
            .map(|call| {
              (
                call.id.clone(),
                call.function.name.clone(),
                call.function.arguments.clone(),
                call.preview.clone(),
              )
            })
            .unwrap_or_default();
          let is_subagent = runs_subagent(&name);
          let task = if is_subagent {
            subagent_task(&arguments)
          } else {
            None
          };
          let mut item =
            MessageItem::new_tool_call(id, name, arguments, preview);
          let mut report = None;
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
                if is_subagent {
                  report = Some((content.to_string(), error));
                } else {
                  item.push_tool_result(content.to_string(), error);
                }
                i = j;
                break;
              }
            }
          }
          self.messages.push(item);
          if let Some(task) = task {
            self.add_subagent_task(&task);
          }
          if let Some((content, error)) = report
            && !content.trim().is_empty()
          {
            let message = MessageItem::new(content, Role::SubagentReport)
              .with_error(error)
              .with_depth(1);
            self.messages.push(message);
          }
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

fn subagent_task(arguments: &str) -> Option<String> {
  let mut params = parse_tool_params(arguments);
  let task = params.remove("task").or_else(|| params.remove("goal"))?;
  if task.trim().is_empty() {
    return None;
  }
  return Some(task);
}
