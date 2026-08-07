use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::slash_commands::SlashCommand;
use crate::tui::popup::{Popup, constrained_width};

pub struct HelpPopup {
  popup: Popup,
  commands: Vec<SlashCommand>,
}

impl HelpPopup {
  pub fn new() -> Self {
    return Self {
      popup: Popup::new(),
      commands: SlashCommand::all().to_vec(),
    };
  }

  pub fn draw(&self, frame: &mut Frame) {
    let num_commands = self.commands.len();
    let popup_height = 4u16 + num_commands as u16;
    let popup_area = frame.area().centered(
      Constraint::Length(constrained_width(frame.area().width)),
      Constraint::Length(popup_height),
    );
    let lines: Vec<Line> = self
      .commands
      .iter()
      .map(|cmd| Line::from(format!("{} - {}", cmd.name(), cmd.description())))
      .collect();
    let paragraph = Paragraph::new(lines);
    if let Some(content_area) =
      self
        .popup
        .draw_header(frame, "Available Commands", popup_area)
    {
      frame.render_widget(paragraph, content_area);
    }
  }

  pub fn handle_input(&mut self) {
    if self.popup.is_open() {
      self.close_help_popup();
    }
  }

  pub fn open_help_popup(&mut self, commands: Vec<SlashCommand>) {
    self.commands = commands;
    self.popup.open_popup();
  }

  fn close_help_popup(&mut self) {
    self.popup.close_popup();
  }

  pub fn is_open(&self) -> bool {
    return self.popup.is_open();
  }
}
