mod input;
mod statusline;

use ratatui::Frame;
use ratatui::crossterm::event::{self, KeyCode};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Paragraph};

use crate::slash_commands::SlashCommand;
use crate::tui::input::Input;
use crate::tui::statusline::StatusLine;

pub struct TUI {
  input: Input,
  messages: Vec<String>,
  running: bool,
  show_help_popup: bool,
}

impl TUI {
  pub fn new() -> Self {
    Self {
      input: Input::new(),
      messages: Vec::new(),
      running: true,
      show_help_popup: false,
    }
  }

  pub fn is_running(&self) -> bool {
    return self.running;
  }

  pub fn draw(&self, frame: &mut Frame) {
    let layout = Layout::vertical([
      Constraint::Min(1),
      Constraint::Length(3),
      Constraint::Length(1),
    ]);
    let [messages_area, input_area, status_bar_area] =
      frame.area().layout(&layout);

    let messages: Vec<ListItem> = self
      .messages
      .iter()
      .map(|m| {
        let content = Line::from(Span::raw(m.clone()));
        ListItem::new(content)
      })
      .collect();
    let messages = List::new(messages);
    frame.render_widget(messages, messages_area);

    let input = Paragraph::new(self.input.content.as_str())
      .style(Style::default())
      .block(Block::bordered());
    frame.render_widget(input, input_area);

    let statusline = StatusLine::new();
    frame
      .render_widget(statusline.draw(status_bar_area.width), status_bar_area);

    frame.set_cursor_position(Position::new(
      input_area.x + self.input.character_index as u16 + 1,
      input_area.y + 1,
    ));

    if self.show_help_popup {
      let num_commands = SlashCommand::all().len();
      let popup_height = 3u16 + num_commands as u16;
      let popup_y = messages_area.y + messages_area.height - popup_height;
      let popup_area =
        Rect::new(messages_area.x, popup_y, messages_area.width, popup_height);
      let lines: Vec<Line> = [Line::from(Span::raw("Available commands:"))]
        .into_iter()
        .chain(SlashCommand::all().iter().map(|cmd| {
          Line::from(format!("  {} - {}", cmd.name(), cmd.description()))
        }))
        .collect();
      let paragraph = Paragraph::new(lines).block(Block::bordered());
      frame.render_widget(paragraph, popup_area);
    }
  }

  pub async fn read_input(&mut self) -> Option<String> {
    let event = match event::read() {
      Ok(e) => e,
      Err(e) => {
        eprintln!("TUI event error: {}", e);
        return None;
      }
    };

    if let Some(key) = event.as_key_press_event() {
      if self.show_help_popup {
        self.close_help_popup();
      }

      match key.code {
        KeyCode::Enter => {
          let input = self.input.submit_content();
          if input.is_empty() {
            return None;
          }
          return Some(input);
        }
        KeyCode::Char(to_insert) => self.input.enter_char(to_insert),
        KeyCode::Backspace => self.input.delete_char(),
        KeyCode::Left => self.input.move_cursor_left(),
        KeyCode::Right => self.input.move_cursor_right(),
        KeyCode::Esc => {
          self.running = false;
          return None;
        }
        _ => {}
      };
    }
    None
  }

  pub fn add_message(&mut self, message: String) {
    self.messages.push(message);
  }

  pub fn open_help_popup(&mut self) {
    self.show_help_popup = true;
  }

  fn close_help_popup(&mut self) {
    self.show_help_popup = false;
  }

  pub fn quit(&mut self) {
    self.running = false;
  }
}
