use ratatui::Frame;
use ratatui::crossterm::event::{self, KeyCode};
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Paragraph};

use crate::app::App;
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::prompt::Prompt;
use crate::tui::input::Input;
use crate::vlog;

impl App {
  pub async fn run_tui(&mut self) -> RuntimeResult<String> {
    let prompt = Prompt::new();
    vlog!("Prompt: {}", prompt.get_system_prompt());

    return ratatui::run(|terminal| -> RuntimeResult<String> {
      let mut tui = TUI::new();
      while tui.running {
        terminal
          .draw(|frame| tui.render(frame))
          .map_err(|e| RuntimeError::TUI(e.to_string()))?;
        tui.handle_events()?;
      }
      return Ok(String::new());
    });
  }
}

struct TUI {
  input: Input,
  messages: Vec<String>,
  running: bool,
}

impl TUI {
  fn new() -> Self {
    let input = Input::new();
    return Self {
      input,
      messages: Vec::new(),
      running: true,
    };
  }

  fn submit_message(&mut self) {
    let input = self.input.submit_content();

    self.messages.push(input);
  }

  fn render(&self, frame: &mut Frame) {
    let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]);
    let [messages_area, input_area] = frame.area().layout(&layout);

    let messages: Vec<ListItem> = self
      .messages
      .iter()
      .enumerate()
      .map(|(i, m)| {
        let content = Line::from(Span::raw(format!("{i}: {m}")));
        ListItem::new(content)
      })
      .collect();
    let messages = List::new(messages).block(Block::bordered());
    frame.render_widget(messages, messages_area);

    let input = Paragraph::new(self.input.content.as_str())
      .style(Style::default())
      .block(Block::bordered());
    frame.render_widget(input, input_area);

    frame.set_cursor_position(Position::new(
      input_area.x + self.input.character_index as u16 + 1,
      input_area.y + 1,
    ));
  }

  fn handle_events(&mut self) -> RuntimeResult<String> {
    let event = event::read().map_err(|e| RuntimeError::TUI(e.to_string()))?;

    if let Some(key) = event.as_key_press_event() {
      match key.code {
        KeyCode::Enter => self.submit_message(),
        KeyCode::Char(to_insert) => self.input.enter_char(to_insert),
        KeyCode::Backspace => self.input.delete_char(),
        KeyCode::Left => self.input.move_cursor_left(),
        KeyCode::Right => self.input.move_cursor_right(),
        KeyCode::Esc => self.quit(),
        _ => {}
      };
    };
    return Ok(String::new());
  }

  pub fn quit(&mut self) {
    self.running = false;
  }
}
