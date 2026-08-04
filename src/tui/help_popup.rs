use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use crate::slash_commands::SlashCommand;
use crate::tui::popup::Popup;

pub struct HelpPopup {
  popup: Popup,
}

impl HelpPopup {
  pub fn new() -> Self {
    return Self {
      popup: Popup::new(),
    };
  }

  pub fn draw(&self, frame: &mut Frame, messages_area: Rect) {
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
    let paragraph = Paragraph::new(lines)
      .block(Block::bordered().border_type(BorderType::Rounded));
    self.popup.draw(frame, paragraph, popup_area);
  }

  pub fn handle_input(&mut self) {
    if self.popup.is_open() {
      self.close_help_popup();
    }
  }

  pub fn open_help_popup(&mut self) {
    self.popup.open_popup();
  }

  fn close_help_popup(&mut self) {
    self.popup.close_popup();
  }
}
