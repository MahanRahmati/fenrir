mod button;
mod completion_popup;
mod confirm_popup;
mod container;
mod help_popup;
mod history;
mod icon;
mod input;
mod message_item;
mod messages_list;
mod models_popup;
mod personas_popup;
mod popup;
mod popup_list;
mod popup_search;
mod queued_messages_list;
mod sessions_popup;
mod sessions_sidebar;
mod sidebar;
mod spinner;
mod statusline;
mod tool_view;
mod view;

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::Position;
use uuid::Uuid;

use crate::tui::completion_popup::CompletionPopup;
use crate::tui::confirm_popup::ConfirmPopup;
use crate::tui::help_popup::HelpPopup;
use crate::tui::history::PromptHistory;
use crate::tui::input::Input;
use crate::tui::messages_list::MessagesList;
use crate::tui::models_popup::ModelsPopup;
use crate::tui::personas_popup::PersonasPopup;
use crate::tui::queued_messages_list::QueueMessagesList;
use crate::tui::sessions_popup::SessionsPopup;
use crate::tui::sessions_sidebar::SessionsSidebar;
use crate::tui::statusline::StatusLine;
use fenrir_core::engine::protocol::{
  CommandInfo, PersonaInfo, SessionStatus, SessionSummary,
};
use fenrir_core::provider_manager::ChatUsage;
use fenrir_core::provider_manager::Model;
use fenrir_core::session_manager::Message;
use fenrir_core::tool_manager::ToolPreview;

const TOP_PADDING: u16 = 1;
const HORIZONTAL_PADDING: u16 = 2;

pub struct Tui {
  input: Input,
  messages_list: MessagesList,
  queued_messages_list: QueueMessagesList,
  running: bool,
  help_popup: HelpPopup,
  sessions_popup: SessionsPopup,
  sessions_sidebar: SessionsSidebar,
  pending_session: Option<Uuid>,
  personas_popup: PersonasPopup,
  models_popup: ModelsPopup,
  confirm_popup: ConfirmPopup,
  completion_popup: CompletionPopup,
  statusline: StatusLine,
  history: PromptHistory,
  history_enabled: bool,
  cancel_requested: bool,
  last_escape_time: Option<Instant>,
}

impl Tui {
  pub fn new(
    cwd: &str,
    history_enabled: bool,
    expanded_reasoning: bool,
  ) -> Self {
    return Self {
      input: Input::new(),
      messages_list: MessagesList::new(expanded_reasoning),
      queued_messages_list: QueueMessagesList::new(),
      running: true,
      help_popup: HelpPopup::new(),
      sessions_popup: SessionsPopup::new(),
      sessions_sidebar: SessionsSidebar::new(),
      pending_session: None,
      personas_popup: PersonasPopup::new(),
      models_popup: ModelsPopup::new(),
      confirm_popup: ConfirmPopup::new(),
      completion_popup: CompletionPopup::new(cwd),
      statusline: StatusLine::new(cwd),
      history: PromptHistory::new(),
      history_enabled,
      cancel_requested: false,
      last_escape_time: None,
    };
  }

  pub fn set_usage(
    &mut self,
    usage: Option<ChatUsage>,
    context_window: Option<u64>,
  ) {
    self.statusline.set_usage(usage, context_window);
  }

  pub fn set_processing(&mut self, processing: bool) {
    self.statusline.set_processing(processing);
  }

  pub fn start_working(&mut self, started_at: i64) {
    self.messages_list.start_working(started_at);
  }

  pub fn finish_working(&mut self, duration_seconds: u64) {
    self.messages_list.finish_working(duration_seconds);
  }

  pub fn clear_working(&mut self) {
    self.messages_list.clear_working();
  }

  pub fn set_retrying(
    &mut self,
    attempt: u32,
    max_attempts: u32,
    delay_seconds: u64,
  ) {
    self
      .statusline
      .set_retrying(attempt, max_attempts, delay_seconds);
  }

  pub fn clear_retrying(&mut self) {
    self.statusline.clear_retrying();
  }

  pub fn update_subagents(&mut self, delta: i32) {
    self.statusline.update_subagents(delta);
  }

  pub fn reset_subagents(&mut self) {
    self.statusline.reset_subagents();
  }

  pub fn take_cancel_request(&mut self) -> bool {
    let requested = self.cancel_requested;
    self.cancel_requested = false;
    return requested;
  }

  pub fn set_persona(&mut self, name: &str) {
    self.input.set_persona(name);
  }

  pub fn set_model(&mut self, name: &str) {
    self.input.set_model(name);
  }

  pub fn set_history(&mut self, entries: Vec<String>) {
    if !self.history_enabled {
      return;
    }
    self.history.replace(entries);
  }

  pub fn push_history(&mut self, content: String) {
    if !self.history_enabled {
      return;
    }
    self.history.push(content);
  }

  pub fn set_suggestion(&mut self, suggestion: String) {
    self.input.set_suggestion(suggestion);
  }

  pub fn clear_suggestion(&mut self) {
    self.input.clear_suggestion();
  }

  pub fn has_suggestion(&self) -> bool {
    return self.input.has_suggestion();
  }

  pub fn input_is_empty(&self) -> bool {
    return self.input.content.is_empty();
  }

  pub fn is_running(&self) -> bool {
    return self.running;
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let has_messages = !self.messages_list.is_empty();
    let has_queued_messages = !self.queued_messages_list.is_empty();
    let areas = view::layout(
      frame.area(),
      &mut self.input,
      &self.queued_messages_list,
      has_messages,
      has_queued_messages,
      self.sessions_sidebar.is_visible(),
    );

    if let Some(sidebar_area) = areas.sidebar {
      self.sessions_sidebar.draw(frame, sidebar_area);
    } else {
      self.sessions_sidebar.clear_area();
      self.sessions_sidebar.set_focused(false);
    }

    if has_messages {
      self.messages_list.draw(frame, areas.messages);
    }

    if has_queued_messages {
      self.queued_messages_list.draw(frame, areas.queued_messages);
    }

    self.completion_popup.draw(frame, areas.input);

    self.input.update_scroll_for_height(areas.input.height);
    frame.render_widget(self.input.widget(areas.input.height), areas.input);

    frame.render_widget(
      self
        .statusline
        .draw(areas.status_bar.width, areas.sidebar.is_some()),
      areas.status_bar,
    );

    let sidebar_focused =
      self.sessions_sidebar.is_shown() && self.sessions_sidebar.is_focused();
    if !self.any_popup_open() && !sidebar_focused {
      let (cursor_row, cursor_col) = self.input.cursor_position();
      frame.set_cursor_position(Position::new(
        areas.input.x + cursor_col as u16 + 2,
        areas.input.y + (cursor_row - self.input.scroll_offset) as u16 + 1,
      ));
    }

    self.sessions_popup.draw(frame);
    self.personas_popup.draw(frame);
    self.models_popup.draw(frame);
    self.help_popup.draw(frame);
    self.confirm_popup.draw(frame);
  }

  fn any_popup_open(&self) -> bool {
    return self.sessions_popup.is_open()
      || self.personas_popup.is_open()
      || self.models_popup.is_open()
      || self.help_popup.is_open()
      || self.confirm_popup.is_open()
      || self.completion_popup.is_open();
  }

  pub async fn handle_event(&mut self, event: Event) -> Option<String> {
    if let Event::Mouse(mouse) = &event {
      if !self.any_popup_open()
        && self.sessions_sidebar.contains(mouse.column, mouse.row)
      {
        if let Some(session_id) = self.sessions_sidebar.handle_mouse(mouse) {
          self.pending_session = Some(session_id);
          self.sessions_sidebar.set_focused(false);
        }
      } else {
        self.messages_list.handle_mouse(mouse);
      }
    }

    if let Some(key) = event.as_key_press_event() {
      let is_shift_enter = key.code == KeyCode::Enter
        && key.modifiers.contains(KeyModifiers::SHIFT);

      if self.confirm_popup.handle_input(key.code) {
        return None;
      }
      if self.help_popup.handle_input(key.code) {
        return None;
      }
      if self.sessions_popup.handle_input(key.code) {
        return None;
      }
      if self.personas_popup.handle_input(key.code) {
        return None;
      }
      if self.models_popup.handle_input(key.code) {
        return None;
      }

      if key.code == KeyCode::Char('b')
        && key.modifiers.contains(KeyModifiers::CONTROL)
      {
        self.sessions_sidebar.toggle();
        if !self.sessions_sidebar.is_visible() {
          self.sessions_sidebar.set_focused(false);
        }
        return None;
      }

      if self.sessions_sidebar.is_focused() {
        match key.code {
          KeyCode::Esc | KeyCode::Tab => {
            self.sessions_sidebar.set_focused(false);
            return None;
          }
          KeyCode::Up
          | KeyCode::Down
          | KeyCode::PageUp
          | KeyCode::PageDown
          | KeyCode::Home
          | KeyCode::End => {
            self.sessions_sidebar.handle_key(key.code);
            return None;
          }
          KeyCode::Enter => {
            if let Some(session_id) = self.sessions_sidebar.selected_session() {
              self.pending_session = Some(session_id);
            }
            self.sessions_sidebar.set_focused(false);
            return None;
          }
          _ => self.sessions_sidebar.set_focused(false),
        }
      }

      if is_shift_enter {
        self.history.reset();
        self.input.enter_char('\n');
        self.completion_popup.refresh(&self.input).await;
        return None;
      }

      if key.code == KeyCode::Tab && self.input.accept_suggestion() {
        self.history.reset();
        return None;
      }

      if self.completion_popup.is_open()
        && self
          .completion_popup
          .handle_input(key.code, &mut self.input)
          .await
      {
        return None;
      }

      if key.code == KeyCode::Tab && self.sessions_sidebar.is_shown() {
        self.sessions_sidebar.set_focused(true);
        return None;
      }

      if key.code == KeyCode::Esc {
        self.input.clear_suggestion();
        let now = Instant::now();
        if let Some(last) = self.last_escape_time
          && now.duration_since(last) < Duration::from_secs(1)
        {
          self.cancel_requested = true;
          self.last_escape_time = None;
        } else {
          self.last_escape_time = Some(now);
        }
        return None;
      }

      match key.code {
        KeyCode::Enter => {
          let input = self.input.submit_content();
          self.history.reset();
          if input.is_empty() {
            return None;
          }
          return Some(input);
        }
        KeyCode::Char(to_insert) => {
          self.history.reset();
          self.input.enter_char(to_insert);
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::Backspace => {
          self.history.reset();
          self.input.delete_char();
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::Left => {
          self.history.disarm();
          self.input.move_cursor_left();
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::Right => {
          self.history.disarm();
          self.input.move_cursor_right();
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::Up
          if (self.history.is_fresh() || self.input.at_start())
            && self.history.previous(&mut self.input) => {}
        KeyCode::Up => {
          self.history.disarm();
          self.input.move_cursor_up();
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::Down
          if (self.history.is_fresh() || self.input.at_end())
            && self.history.next(&mut self.input) => {}
        KeyCode::Down => {
          self.history.disarm();
          self.input.move_cursor_down();
          self.completion_popup.refresh(&self.input).await;
        }
        KeyCode::PageUp => {
          self.messages_list.scroll_page_older();
        }
        KeyCode::PageDown => {
          self.messages_list.scroll_page_newer();
        }
        _ => {}
      };
    }
    return None;
  }

  pub fn add_message(&mut self, message: String) {
    self.messages_list.add_message(message);
  }

  pub fn add_error(&mut self, message: String) {
    self.messages_list.add_error(message);
  }

  pub fn add_tool_call(
    &mut self,
    id: String,
    name: String,
    arguments: String,
    preview: Option<ToolPreview>,
  ) {
    self
      .messages_list
      .add_tool_call(id, name, arguments, preview);
  }

  pub fn add_tool_result(&mut self, id: String, result: String, error: bool) {
    self.messages_list.add_tool_result(id, result, error);
  }

  pub fn add_subagent_token(
    &mut self,
    tool_call_id: &str,
    text: &str,
    reasoning: bool,
  ) {
    self
      .messages_list
      .add_subagent_token(tool_call_id, text, reasoning);
  }

  pub fn add_subagent_tool_call(
    &mut self,
    tool_call_id: &str,
    id: String,
    name: String,
    arguments: String,
    preview: Option<ToolPreview>,
  ) {
    self.messages_list.add_subagent_tool_call(
      tool_call_id,
      id,
      name,
      arguments,
      preview,
    );
  }

  pub fn add_subagent_tool_result(
    &mut self,
    tool_call_id: &str,
    id: String,
    result: String,
    error: bool,
  ) {
    self.messages_list.add_subagent_tool_result(
      tool_call_id,
      id,
      result,
      error,
    );
  }

  pub fn add_subagent_retry(
    &mut self,
    tool_call_id: &str,
    attempt: u32,
    max_attempts: u32,
    delay_seconds: u64,
  ) {
    self.messages_list.add_subagent_retry(
      tool_call_id,
      attempt,
      max_attempts,
      delay_seconds,
    );
  }

  pub fn add_summary(&mut self, summary: String) {
    self.messages_list.add_summary(summary);
  }

  pub fn ensure_reasoning_line(&mut self) {
    self.messages_list.ensure_reasoning_line();
  }

  pub fn finish_reasoning(&mut self) {
    self.messages_list.finish_reasoning();
  }

  pub fn ensure_content_line(&mut self) {
    self.messages_list.ensure_content_line();
  }

  pub fn append_to_last_message(&mut self, text: &str) {
    self.messages_list.append_to_last_message(text);
  }

  pub fn update_messages(&mut self, messages: Option<&[Message]>) {
    self.clear_suggestion();
    self.messages_list.update_messages(messages);
  }

  pub fn update_queued_messages(&mut self, queued_messages: Vec<String>) {
    self.queued_messages_list.update_messages(queued_messages);
  }

  pub fn open_help_popup(&mut self, commands: Vec<CommandInfo>) {
    self.help_popup.open_help_popup(commands);
  }

  pub fn open_sessions_popup(
    &mut self,
    sessions: Vec<SessionSummary>,
    current_session: Option<Uuid>,
  ) {
    self
      .sessions_popup
      .open_sessions_popup(sessions, current_session);
  }

  pub fn get_selected_session(&mut self) -> Option<Uuid> {
    return self.sessions_popup.get_selected_session();
  }

  pub fn sessions_popup_open(&self) -> bool {
    return self.sessions_popup.is_open();
  }

  pub fn update_sessions(&mut self, sessions: Vec<SessionSummary>) {
    self.sessions_sidebar.update_sessions(sessions);
  }

  pub fn update_session_status(
    &mut self,
    session_id: Uuid,
    status: SessionStatus,
  ) {
    self
      .sessions_sidebar
      .update_session_status(session_id, status);
  }

  pub fn sync_session(&mut self, summary: &SessionSummary) {
    self.sessions_sidebar.sync_session(summary.clone());
  }

  pub fn update_session_title(&mut self, session_id: Uuid, title: String) {
    self.sessions_sidebar.update_title(session_id, title);
  }

  pub fn touch_session(&mut self, session_id: Uuid) {
    self.sessions_sidebar.touch_session(session_id);
  }

  pub fn take_session_request(&mut self) -> Option<Uuid> {
    return self.pending_session.take();
  }

  pub fn open_personas_popup(
    &mut self,
    personas: &[PersonaInfo],
    current_persona: Option<&str>,
  ) {
    self
      .personas_popup
      .open_personas_popup(personas, current_persona);
  }

  pub fn get_selected_persona(&mut self) -> Option<String> {
    return self.personas_popup.get_selected_persona();
  }

  pub fn personas_popup_open(&self) -> bool {
    return self.personas_popup.is_open();
  }

  pub fn open_models_popup(
    &mut self,
    models: Vec<Model>,
    current: Option<Model>,
  ) {
    self.models_popup.open_models_popup(models, current);
  }

  pub fn models_popup_open(&self) -> bool {
    return self.models_popup.is_open();
  }

  pub fn get_selected_model(&mut self) -> Option<Model> {
    return self.models_popup.get_selected_model();
  }

  pub fn open_confirm_popup(&mut self, message: String) {
    self.confirm_popup.open_confirm_popup(message);
  }

  pub fn close_confirm_popup(&mut self) {
    self.confirm_popup.close();
  }

  pub fn get_confirmation(&mut self) -> bool {
    return self.confirm_popup.get_confirmation();
  }

  pub fn confirm_popup_open(&self) -> bool {
    return self.confirm_popup.is_open();
  }

  pub fn quit(&mut self) {
    self.running = false;
  }
}
