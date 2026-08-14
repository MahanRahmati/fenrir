use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Constraint;
use ratatui::text::Line;
use ratatui::widgets::{
  Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};

use crate::slash_commands::SlashCommand;
use crate::tui::popup::{Popup, constrained_height, constrained_width};

const MAX_VISIBLE_ITEMS: u16 = 10;

pub struct HelpPopup {
  popup: Popup,
  commands: Vec<SlashCommand>,
  scroll_offset: usize,
}

impl HelpPopup {
  pub fn new() -> Self {
    return Self {
      popup: Popup::new(),
      commands: SlashCommand::all().to_vec(),
      scroll_offset: 0,
    };
  }

  pub fn draw(&self, frame: &mut Frame) {
    if !self.popup.is_open() {
      return;
    }

    let area = frame.area();
    let num_commands = self.commands.len();
    let popup_height =
      constrained_height(num_commands as u16, MAX_VISIBLE_ITEMS) + 4;
    let popup_area = area.centered(
      Constraint::Length(constrained_width(area.width)),
      Constraint::Length(popup_height.min(area.height)),
    );
    let visible_rows = popup_height.saturating_sub(4) as usize;
    let lines: Vec<Line> = self
      .commands
      .iter()
      .skip(self.scroll_offset)
      .take(visible_rows)
      .map(|cmd| Line::from(format!("{} - {}", cmd.name(), cmd.description())))
      .collect();
    let paragraph = Paragraph::new(lines);
    if let Some(content_area) =
      self
        .popup
        .draw_header(frame, "Available Commands", popup_area)
    {
      frame.render_widget(paragraph, content_area);
      if num_commands > visible_rows {
        let mut scrollbar_state =
          ScrollbarState::new(num_commands).position(self.scroll_offset);
        frame.render_stateful_widget(
          Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None),
          content_area,
          &mut scrollbar_state,
        );
      }
    }
  }

  pub fn handle_input(&mut self, key: KeyCode) -> bool {
    if !self.popup.is_open() {
      return false;
    }

    let num_commands = self.commands.len();
    let visible_rows = num_commands.min(MAX_VISIBLE_ITEMS as usize);
    if num_commands <= visible_rows {
      self.close_help_popup();
      return true;
    }
    match key {
      KeyCode::Up => {
        if self.scroll_offset > 0 {
          self.scroll_offset -= 1;
        }
      }
      KeyCode::Down => {
        if self.scroll_offset + visible_rows < num_commands {
          self.scroll_offset += 1;
        }
      }
      _ => self.close_help_popup(),
    }
    return true;
  }

  pub fn open_help_popup(&mut self, commands: Vec<SlashCommand>) {
    self.commands = commands;
    self.scroll_offset = 0;
    self.popup.open_popup();
  }

  fn close_help_popup(&mut self) {
    self.popup.close_popup();
  }

  pub fn is_open(&self) -> bool {
    return self.popup.is_open();
  }
}
