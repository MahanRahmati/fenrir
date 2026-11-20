use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolPreview {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub summary: Option<String>,
  pub lines: Vec<ToolPreviewLine>,
}

impl ToolPreview {
  pub fn with_summary(
    lines: Vec<ToolPreviewLine>,
    summary: impl Into<String>,
  ) -> Self {
    return Self {
      summary: Some(summary.into()),
      lines,
    };
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolPreviewLine {
  pub kind: ToolPreviewLineKind,
  pub text: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub line_number: Option<usize>,
}

impl ToolPreviewLine {
  pub fn new(kind: ToolPreviewLineKind, text: impl Into<String>) -> Self {
    return Self {
      kind,
      text: text.into(),
      line_number: None,
    };
  }

  pub fn numbered(
    kind: ToolPreviewLineKind,
    line_number: usize,
    text: impl Into<String>,
  ) -> Self {
    return Self {
      kind,
      text: text.into(),
      line_number: Some(line_number),
    };
  }

  pub fn plain(text: impl Into<String>) -> Self {
    return Self::new(ToolPreviewLineKind::Plain, text);
  }

  pub fn header(text: impl Into<String>) -> Self {
    return Self::new(ToolPreviewLineKind::Header, text);
  }

  pub fn hunk() -> Self {
    return Self::new(ToolPreviewLineKind::Hunk, String::new());
  }

  pub fn warning(text: impl Into<String>) -> Self {
    return Self::new(ToolPreviewLineKind::Warning, text);
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPreviewLineKind {
  Plain,
  Context,
  Added,
  Removed,
  Header,
  Hunk,
  Warning,
}
