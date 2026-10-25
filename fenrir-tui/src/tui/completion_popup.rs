use std::collections::HashSet;

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};

use crate::tui::input::Input;
use crate::tui::popup::Popup;
use fenrir_core::files::operations::is_dir;
use fenrir_core::slash_commands::SlashCommand;
use fenrir_core::utils::process;

const MAX_RESULTS: usize = 10;
const MAX_POPUP_HEIGHT: usize = 10;

#[derive(Clone, Copy, PartialEq)]
enum CompletionKind {
  File,
  SlashCommand,
}

struct CompletionItem {
  text: String,
  is_dir: bool,
  detail: Option<String>,
}

pub struct CompletionPopup {
  popup: Popup,
  kind: Option<CompletionKind>,
  candidates: Vec<CompletionItem>,
  selected_index: usize,
  token_start: usize,
}

impl CompletionPopup {
  pub fn new() -> Self {
    return Self {
      popup: Popup::new(),
      kind: None,
      candidates: Vec::new(),
      selected_index: 0,
      token_start: 0,
    };
  }

  pub fn is_open(&self) -> bool {
    return self.popup.is_open();
  }

  pub async fn handle_input(
    &mut self,
    key: KeyCode,
    input: &mut Input,
  ) -> bool {
    if !self.is_open() {
      return false;
    }

    match key {
      KeyCode::Enter => return self.accept(input).await,
      KeyCode::Up if self.selected_index > 0 => {
        self.selected_index -= 1;
      }
      KeyCode::Down
        if !self.candidates.is_empty()
          && self.selected_index < self.candidates.len() - 1 =>
      {
        self.selected_index += 1;
      }
      KeyCode::Esc => self.close(),
      KeyCode::Char(to_insert) => {
        input.enter_char(to_insert);
        self.refresh(input).await;
      }
      KeyCode::Backspace => {
        input.delete_char();
        self.refresh(input).await;
      }
      KeyCode::Left => {
        input.move_cursor_left();
        self.refresh(input).await;
      }
      KeyCode::Right => {
        input.move_cursor_right();
        self.refresh(input).await;
      }
      _ => {}
    }
    return true;
  }

  pub async fn refresh(&mut self, input: &Input) {
    let (start, token) =
      match current_token(&input.content, input.character_index) {
        Some(token) => token,
        None => {
          self.close();
          return;
        }
      };

    let kind = if token.starts_with('@') {
      Some(CompletionKind::File)
    } else if token.starts_with('/') {
      Some(CompletionKind::SlashCommand)
    } else {
      None
    };

    let kind = match kind {
      Some(kind) => kind,
      None => {
        self.close();
        return;
      }
    };

    let candidates = match kind {
      CompletionKind::File => file_candidates(&token[1..]).await,
      CompletionKind::SlashCommand => command_candidates(&token[1..]),
    };
    if candidates.is_empty() {
      self.close();
      return;
    }

    self.kind = Some(kind);
    self.token_start = start;
    self.candidates = candidates;
    self.selected_index = 0;
    self.popup.open_popup();
  }

  pub fn close(&mut self) {
    self.kind = None;
    self.candidates.clear();
    self.selected_index = 0;
    self.popup.close_popup();
  }

  pub fn draw(&mut self, frame: &mut Frame, input_area: Rect) {
    if !self.is_open() {
      return;
    }

    let height = (self.candidates.len().min(MAX_POPUP_HEIGHT) as u16)
      .saturating_add(4)
      .min(input_area.y);
    let popup_area = Rect::new(
      input_area.x,
      input_area.y.saturating_sub(height),
      input_area.width,
      height,
    );

    let title = match self.kind {
      Some(CompletionKind::File) => "Files",
      Some(CompletionKind::SlashCommand) => "Commands",
      None => return,
    };

    if let Some(list_area) = self.popup.draw_header(frame, title, popup_area) {
      let items: Vec<ListItem> = self
        .candidates
        .iter()
        .map(|item| self.build_list_item(item))
        .collect();
      let list = List::new(items).highlight_style(Style::default().reversed());
      let mut list_state = ListState::default();
      list_state.select(Some(self.selected_index));
      frame.render_stateful_widget(list, list_area, &mut list_state);
    }
  }

  fn build_list_item(&self, item: &CompletionItem) -> ListItem<'static> {
    let mut spans = match self.kind {
      Some(CompletionKind::File) => {
        let display =
          format!("{}{}", item.text, if item.is_dir { "/" } else { "" });
        vec![Span::raw(display)]
      }
      Some(CompletionKind::SlashCommand) => {
        vec![Span::raw(format!("/{}", item.text))]
      }
      None => vec![Span::raw(item.text.clone())],
    };
    if let Some(detail) = &item.detail {
      spans.push(Span::raw(format!("  {detail}")));
    }
    return ListItem::new(Line::from(spans));
  }

  async fn accept(&mut self, input: &mut Input) -> bool {
    let item = match self.candidates.get(self.selected_index) {
      Some(item) => item,
      None => {
        self.close();
        return true;
      }
    };

    let replacement = match self.kind {
      Some(CompletionKind::File) => {
        if item.is_dir {
          format!("@{}/", item.text)
        } else {
          format!("@{} ", item.text)
        }
      }
      Some(CompletionKind::SlashCommand) => format!("/{}", item.text),
      None => return true,
    };

    let cursor_byte = byte_index(&input.content, input.character_index);
    let current = &input.content[self.token_start..cursor_byte];
    if current == replacement {
      self.close();
      return false;
    }

    input
      .content
      .replace_range(self.token_start..cursor_byte, &replacement);
    input.character_index = input.content[..self.token_start].chars().count()
      + replacement.chars().count();

    return match self.kind {
      Some(CompletionKind::SlashCommand) => {
        self.close();
        false
      }
      _ => {
        if item.is_dir {
          self.refresh(input).await;
        } else {
          self.close();
        }
        true
      }
    };
  }
}

fn current_token(
  content: &str,
  character_index: usize,
) -> Option<(usize, &str)> {
  let cursor_byte = byte_index(content, character_index);
  let mut start = 0;
  for (i, c) in content.char_indices() {
    if i >= cursor_byte {
      break;
    }
    if c.is_whitespace() {
      start = i + c.len_utf8();
    }
  }
  if start >= cursor_byte {
    return None;
  }
  return Some((start, &content[start..cursor_byte]));
}

fn byte_index(content: &str, character_index: usize) -> usize {
  return content
    .char_indices()
    .map(|(i, _)| i)
    .nth(character_index)
    .unwrap_or(content.len());
}

async fn run_fd(args: &[&str]) -> Option<Vec<String>> {
  let output = process::run("fd", args).await.ok()?;
  if !output.exit_code.is_success() {
    return None;
  }

  let mut lines: Vec<String> = Vec::new();
  for line in output.stdout.lines() {
    let line = line.trim_end_matches('/');
    if !line.is_empty() {
      lines.push(line.to_string());
    }
  }
  return Some(lines);
}

async fn file_candidates(prefix: &str) -> Vec<CompletionItem> {
  let (dir_part, name_part) = match prefix.rsplit_once('/') {
    Some((dir, name)) => (dir, name),
    None => ("", prefix),
  };
  let base = if dir_part.is_empty() { "." } else { dir_part };

  let max_results = MAX_RESULTS.to_string();
  let mut names: Vec<String> = Vec::new();

  if name_part.is_empty() {
    let args: Vec<&str> = vec![
      "--base-directory",
      base,
      "--glob",
      "*",
      "--type",
      "f",
      "--type",
      "d",
      "--max-results",
      max_results.as_str(),
      "--max-depth",
      "1",
    ];
    if let Some(lines) = run_fd(&args).await {
      names.extend(lines);
    }
  } else {
    let glob = format!("{name_part}*");
    let args: Vec<&str> = vec![
      "--base-directory",
      base,
      "--glob",
      glob.as_str(),
      "--type",
      "f",
      "--type",
      "d",
      "--max-results",
      max_results.as_str(),
    ];
    if let Some(lines) = run_fd(&args).await {
      names.extend(lines);
    }
    if dir_part.is_empty() {
      let args: Vec<&str> = vec![
        "--base-directory",
        base,
        "--extension",
        name_part,
        "--type",
        "f",
        "--max-results",
        max_results.as_str(),
      ];
      if let Some(lines) = run_fd(&args).await {
        names.extend(lines);
      }
    }
  }

  let mut seen: HashSet<String> = HashSet::new();
  let mut items: Vec<CompletionItem> = Vec::new();
  for name in names {
    if !seen.insert(name.clone()) {
      continue;
    }
    let text = if dir_part.is_empty() {
      name
    } else {
      format!("{dir_part}/{name}")
    };
    let is_dir = is_dir(&text).await;
    items.push(CompletionItem {
      text,
      is_dir,
      detail: None,
    });
  }

  items.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.text.cmp(&b.text)));
  items.truncate(MAX_RESULTS);
  return items;
}

fn command_candidates(prefix: &str) -> Vec<CompletionItem> {
  let mut items: Vec<CompletionItem> = Vec::new();
  for command in SlashCommand::all() {
    let name = command.name();
    if name.starts_with(&format!("/{prefix}")) {
      items.push(CompletionItem {
        text: name[1..].to_string(),
        is_dir: false,
        detail: Some(command.description().to_string()),
      });
    }
  }
  return items;
}
