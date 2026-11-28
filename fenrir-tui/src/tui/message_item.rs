use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::tui::container::Container;
use crate::tui::markdown;
use crate::tui::tool_view::{ToolCallView, tool_call_view, tool_result_label};
use crate::tui::{HORIZONTAL_PADDING, TOP_PADDING};
use fenrir_core::tool_manager::{
  ToolPreview, ToolPreviewLine, ToolPreviewLineKind,
};

const TEXT_EXTRA_PADDING: u16 = 1;
const GUIDE: &str = "   │ ";
const GUIDE_WIDTH: u16 = 4;

#[derive(PartialEq)]
pub enum Role {
  User,
  Assistant,
  Reasoning,
  ToolCall,
  ToolResult,
  SubagentTask,
  SubagentReport,
  Compaction,
  Duration,
  Error,
}

impl Role {
  fn horizontal_padding(&self) -> u16 {
    return match self {
      Role::User | Role::ToolCall | Role::ToolResult | Role::Compaction => {
        HORIZONTAL_PADDING
      }
      Role::Assistant
      | Role::Reasoning
      | Role::SubagentTask
      | Role::SubagentReport
      | Role::Duration
      | Role::Error => HORIZONTAL_PADDING + TEXT_EXTRA_PADDING,
    };
  }
}

pub struct MessageItem {
  text: String,
  role: Role,
  error: bool,
  depth: u16,
  expanded: bool,
  reasoning_done: bool,
  tool_call_id: Option<String>,
  tool_name: Option<String>,
  tool_view: ToolCallView,
  tool_result: Option<(String, bool)>,
  markdown: markdown::Renderer,
  wrapped: Option<(u16, Vec<Line<'static>>)>,
}

impl MessageItem {
  pub fn new(text: String, role: Role) -> Self {
    return Self {
      text,
      role,
      error: false,
      depth: 0,
      expanded: true,
      reasoning_done: false,
      tool_call_id: None,
      tool_name: None,
      tool_view: ToolCallView::default(),
      tool_result: None,
      markdown: markdown::Renderer::new(),
      wrapped: None,
    };
  }

  pub fn with_error(mut self, error: bool) -> Self {
    self.error = error;
    self.wrapped = None;
    return self;
  }

  pub fn with_depth(mut self, depth: u16) -> Self {
    self.depth = depth;
    self.wrapped = None;
    return self;
  }

  pub fn with_expanded(mut self, expanded: bool) -> Self {
    self.expanded = expanded;
    self.wrapped = None;
    return self;
  }

  pub fn with_reasoning_done(mut self, reasoning_done: bool) -> Self {
    self.reasoning_done = reasoning_done;
    self.wrapped = None;
    return self;
  }

  pub fn depth(&self) -> u16 {
    return self.depth;
  }

  pub fn text(&self) -> &str {
    return &self.text;
  }

  pub fn new_tool_call(
    id: String,
    name: String,
    arguments: String,
    preview: Option<ToolPreview>,
  ) -> Self {
    let tool_view = tool_call_view(&name, &arguments, preview.as_ref());
    return Self {
      text: String::new(),
      role: Role::ToolCall,
      error: false,
      depth: 0,
      expanded: true,
      reasoning_done: false,
      tool_call_id: Some(id),
      tool_name: Some(name),
      tool_view,
      tool_result: None,
      markdown: markdown::Renderer::new(),
      wrapped: None,
    };
  }

  pub fn push_tool_result(&mut self, result: String, error: bool) {
    self.tool_result = Some((result, error));
    self.wrapped = None;
  }

  pub fn is_pending_tool_call(&self, id: &str) -> bool {
    return self.role == Role::ToolCall
      && self.tool_call_id.as_deref() == Some(id)
      && self.tool_result.is_none();
  }

  fn horizontal_padding(&self) -> u16 {
    if self.role == Role::ToolCall {
      return self.role.horizontal_padding() + TEXT_EXTRA_PADDING;
    }
    return self.role.horizontal_padding();
  }

  pub fn lines(&mut self, width: u16) -> &[Line<'static>] {
    let is_cached = self
      .wrapped
      .as_ref()
      .is_some_and(|(cached_width, _)| *cached_width == width);
    if !is_cached {
      let left_padding = if self.depth > 0 {
        0
      } else {
        self.horizontal_padding()
      };
      let right_padding = self.horizontal_padding();
      let guide_width = GUIDE_WIDTH.saturating_mul(self.depth);
      let inner_width = width
        .saturating_sub(left_padding)
        .saturating_sub(right_padding)
        .saturating_sub(guide_width);
      let mut lines = match self.role {
        Role::User => self.user_list_item().render_lines(inner_width),
        Role::Assistant | Role::SubagentTask => {
          self.markdown_lines(inner_width)
        }
        Role::Reasoning => self.reasoning_lines(inner_width),
        Role::ToolCall => self.tool_call_lines(inner_width),
        Role::ToolResult => self.tool_result_lines(inner_width),
        Role::SubagentReport if self.error => {
          self.error_list_item().render_lines(inner_width)
        }
        Role::SubagentReport => self.markdown_lines(inner_width),
        Role::Compaction => self.compaction_lines(inner_width),
        Role::Duration => self.duration_list_item().render_lines(inner_width),
        Role::Error => self.error_list_item().render_lines(inner_width),
      };
      if lines.is_empty() {
        lines.push(Line::default());
      }
      if left_padding > 0 {
        let padding = " ".repeat(left_padding as usize);
        for line in &mut lines {
          line.spans.insert(0, Span::raw(padding.clone()));
        }
      }
      for _ in 0..TOP_PADDING {
        lines.insert(0, Line::default());
      }
      if self.depth > 0 {
        let guide = Span::styled(
          GUIDE.repeat(self.depth as usize),
          Style::default().dim(),
        );
        for line in &mut lines {
          line.spans.insert(0, guide.clone());
        }
      }
      self.wrapped = Some((width, lines));
    }
    return &self
      .wrapped
      .as_ref()
      .expect("wrapped lines are always present")
      .1;
  }

  fn user_list_item(&self) -> Container<'static> {
    return Container::builder(self.text.clone())
      .color(Color::Yellow)
      .build();
  }

  fn reasoning_lines(&mut self, width: u16) -> Vec<Line<'static>> {
    let style = Style::default().dim();
    let label = match (self.reasoning_done, self.expanded) {
      (true, true) => "- Thought",
      (true, false) => "+ Thought",
      (false, true) => "- Thinking",
      (false, false) => "+ Thinking",
    };
    let mut lines = vec![Line::from(Span::styled(label, style))];
    if self.expanded {
      let body = self.markdown.render(&self.text, width);
      let body: Vec<Line<'static>> = body
        .into_iter()
        .map(|line| line.patch_style(style))
        .collect();
      lines.extend(pad_lines(body, width));
    }
    return lines;
  }

  fn markdown_lines(&mut self, width: u16) -> Vec<Line<'static>> {
    let lines = self.markdown.render(&self.text, width);
    return pad_lines(lines, width);
  }

  fn compaction_lines(&self, width: u16) -> Vec<Line<'static>> {
    let inner_width = width.saturating_sub(2);
    let body = markdown::render(&self.text, inner_width);
    return Container::builder(body)
      .title(" Compaction")
      .color(Color::Magenta)
      .trim(false)
      .build()
      .render_lines(width);
  }

  fn error_list_item(&self) -> Container<'static> {
    return Container::builder(self.text.clone())
      .borderless()
      .style(Style::default().fg(Color::Red))
      .build();
  }

  fn duration_list_item(&self) -> Container<'static> {
    return Container::builder(self.text.clone())
      .borderless()
      .style(Style::default().dim())
      .build();
  }

  fn tool_result_lines(&self, width: u16) -> Vec<Line<'static>> {
    return Container::builder(self.text.clone())
      .borderless()
      .style(if self.error {
        Style::default().fg(Color::Red)
      } else {
        Style::default().dim()
      })
      .build()
      .render_lines(width);
  }

  fn tool_call_lines(&self, width: u16) -> Vec<Line<'static>> {
    let view = &self.tool_view;
    let is_success =
      self.tool_result.as_ref().is_some_and(|(_, error)| !*error);
    let icon_style = if is_success {
      Style::default().fg(Color::Green)
    } else {
      Style::default()
    };
    let label = match (&self.tool_name, &self.tool_result) {
      (Some(name), Some((result, false))) => {
        tool_result_label(name, view, result)
          .unwrap_or_else(|| view.label.clone())
      }
      _ => view.label.clone(),
    };
    let icon = format!("{} ", view.icon);
    let mut spans = vec![Span::styled(icon, icon_style)];
    spans.push(Span::raw(truncate_line(&label, width.saturating_sub(2))));
    let mut lines: Vec<Line<'static>> = vec![Line::from(spans)];
    for (index, group) in hunk_groups(&view.body).iter().enumerate() {
      if index > 0 {
        lines.push(Line::default());
      }
      let number_width = gutter_width(group);
      let group_lines: Vec<Line<'static>> = group
        .iter()
        .map(|line| preview_line(line, number_width))
        .collect();
      lines.extend(boxed_lines(group_lines, width));
    }

    if let Some((result, error)) = &self.tool_result
      && (view.show_result || *error)
    {
      let style = if *error {
        Style::default().fg(Color::Red)
      } else {
        Style::default().dim()
      };
      lines.extend(
        result
          .lines()
          .map(|line| Line::from(line.to_string()).patch_style(style)),
      );
    }
    return lines;
  }

  pub fn is_reasoning(&self) -> bool {
    return self.role == Role::Reasoning;
  }

  pub fn finish_reasoning(&mut self) {
    if self.role != Role::Reasoning || self.reasoning_done {
      return;
    }
    self.reasoning_done = true;
    self.expanded = false;
    self.wrapped = None;
  }

  pub fn toggle_expanded(&mut self) {
    self.expanded = !self.expanded;
    self.wrapped = None;
  }

  pub fn is_assistant(&self) -> bool {
    return self.role == Role::Assistant;
  }

  pub fn push_string(&mut self, text: &str) {
    self.text.push_str(text);
    self.wrapped = None;
  }

  pub fn clear_markdown_cache(&mut self) {
    self.markdown.clear();
  }
}

fn pad_lines(lines: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
  let width = width as usize;
  return lines
    .into_iter()
    .map(|mut line| {
      let used = line.width();
      if used < width {
        line.spans.push(Span::raw(" ".repeat(width - used)));
      }
      line
    })
    .collect();
}

fn preview_line(line: &ToolPreviewLine, number_width: usize) -> Line<'static> {
  let mut spans = vec![Span::raw("  ")];

  let marker = match line.kind {
    ToolPreviewLineKind::Added => {
      Some(("+", Style::default().fg(Color::Green)))
    }
    ToolPreviewLineKind::Removed => {
      Some(("-", Style::default().fg(Color::Red)))
    }
    ToolPreviewLineKind::Context => Some((" ", Style::default())),
    _ => None,
  };
  if let Some((marker, style)) = marker {
    spans.push(Span::styled(format!("{marker} "), style));
  }

  if let Some(number) = line.line_number {
    let number_style = match line.kind {
      ToolPreviewLineKind::Added | ToolPreviewLineKind::Plain => {
        Style::default().fg(Color::Green)
      }
      ToolPreviewLineKind::Removed => Style::default().fg(Color::Red),
      _ => Style::default().dim(),
    };
    spans.push(Span::styled(
      format!("{number:>number_width$} "),
      number_style,
    ));
    spans.push(Span::styled("│ ", Style::default().dim()));
  }

  let text_style = match line.kind {
    ToolPreviewLineKind::Warning => Style::default().fg(Color::Yellow),
    ToolPreviewLineKind::Header | ToolPreviewLineKind::Hunk => {
      Style::default().dim()
    }
    _ => Style::default(),
  };
  let text = line.text.replace('\t', "    ");
  spans.push(Span::styled(text, text_style));
  return Line::from(spans);
}

fn hunk_groups(body: &[ToolPreviewLine]) -> Vec<Vec<&ToolPreviewLine>> {
  let mut groups: Vec<Vec<&ToolPreviewLine>> = Vec::new();
  let mut current: Vec<&ToolPreviewLine> = Vec::new();
  for line in body {
    if line.kind == ToolPreviewLineKind::Hunk {
      if !current.is_empty() {
        groups.push(std::mem::take(&mut current));
      }
    } else {
      current.push(line);
    }
  }
  if !current.is_empty() {
    groups.push(current);
  }
  return groups;
}

fn gutter_width(lines: &[&ToolPreviewLine]) -> usize {
  return lines
    .iter()
    .filter_map(|line| line.line_number)
    .map(|number| number.to_string().len())
    .max()
    .unwrap_or(1);
}

fn boxed_lines(lines: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
  let padded: Vec<Line<'static>> = lines
    .into_iter()
    .map(|mut line| {
      line.spans.insert(0, Span::raw(" "));
      line
    })
    .collect();
  return Container::builder(padded)
    .color(Color::DarkGray)
    .trim(false)
    .build()
    .render_lines(width);
}

fn truncate_line(text: &str, width: u16) -> String {
  let width = width as usize;
  let text_len = text.chars().count();
  if text_len <= width {
    return text.to_string();
  }
  if width <= 3 {
    return ".".repeat(width);
  }
  let mut truncated: String = text.chars().take(width - 3).collect();
  truncated.push_str("...");
  return truncated;
}
