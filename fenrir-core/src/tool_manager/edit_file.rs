use anyhow::{Context, Result, bail};
use similar::{ChangeTag, TextDiff};

use crate::files::operations::{read_to_string, write_file};
use crate::tool_manager::helpers::*;
use crate::tool_manager::preview::{
  ToolPreview, ToolPreviewLine, ToolPreviewLineKind,
};
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};

const PREVIEW_CONTEXT_RADIUS: usize = 3;
const MAX_PREVIEW_LINES: usize = 80;

pub const EDIT_FILE_TOOL_NAME: &str = "edit_file";

pub struct EditFileTool;

struct EditPlan {
  path: String,
  old_content: String,
  new_file: String,
  replaced: usize,
  start_line: usize,
  end_line: usize,
  appended: bool,
  new_line_count: usize,
}

impl EditPlan {
  async fn load(params: &ToolParams) -> Result<Self> {
    let path = get_param::<String>(params, EDIT_FILE_TOOL_NAME, "path")?;
    let start_line =
      get_param::<usize>(params, EDIT_FILE_TOOL_NAME, "start_line")?;
    let end_line = get_param::<usize>(params, EDIT_FILE_TOOL_NAME, "end_line")?;
    let new_content =
      get_param::<String>(params, EDIT_FILE_TOOL_NAME, "new_content")?;
    Self::validate_params(start_line, end_line, &new_content)?;
    let content = read_to_string(&path)
      .await
      .context(format!("Cannot read file '{path}'"))?;
    return Self::new(&path, &content, start_line, end_line, &new_content);
  }

  fn validate_params(
    start_line: usize,
    end_line: usize,
    new_content: &str,
  ) -> Result<()> {
    if start_line < 1 {
      bail!("start_line must be >= 1, got {start_line}");
    }
    if end_line < 1 {
      bail!("end_line must be >= 1, got {end_line}");
    }
    if start_line > end_line {
      bail!("start_line ({start_line}) must be <= end_line ({end_line})");
    }
    if new_content.is_empty() {
      bail!("new_content must not be empty");
    }
    return Ok(());
  }

  fn new(
    path: &str,
    content: &str,
    start_line: usize,
    end_line: usize,
    new_content: &str,
  ) -> Result<Self> {
    Self::validate_params(start_line, end_line, new_content)?;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    let trailing_newline = content.ends_with('\n');

    if start_line > total_lines.saturating_add(1) {
      bail!(
        "start_line ({start_line}) is beyond the end of '{path}' \
         ({total_lines} lines)"
      );
    }

    let new_lines: Vec<&str> = new_content.lines().collect();
    let new_line_count = new_lines.len();
    let appended = end_line > total_lines;

    let (mut new_file, replaced) = if appended {
      let mut result = lines;
      result.extend(new_lines);
      let replaced = total_lines.saturating_sub(start_line.saturating_sub(1));
      (result.join("\n"), replaced)
    } else {
      let start_idx = start_line - 1;
      let end_idx = end_line.min(total_lines);
      let mut result: Vec<&str> = Vec::with_capacity(
        total_lines - (end_idx - start_idx) + new_lines.len(),
      );
      result.extend_from_slice(&lines[..start_idx]);
      result.extend_from_slice(&new_lines);
      result.extend_from_slice(&lines[end_idx..]);
      (result.join("\n"), end_idx.saturating_sub(start_line) + 1)
    };

    if trailing_newline {
      new_file.push('\n');
    }

    let edit_plan = Self {
      path: path.to_string(),
      old_content: content.to_string(),
      new_file,
      replaced,
      start_line,
      end_line,
      appended,
      new_line_count,
    };
    return Ok(edit_plan);
  }

  fn preview(&self) -> ToolPreview {
    if self.old_content == self.new_file {
      return ToolPreview::with_summary(
        vec![ToolPreviewLine::warning("No changes")],
        "no changes",
      );
    }

    let diff = TextDiff::from_lines(&self.old_content, &self.new_file);
    let width = diff.old_len().max(diff.new_len()).to_string().len();
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut added = 0;
    let mut removed = 0;
    let mut omitted = 0;
    let mut full = false;
    let hunks: Vec<_> = diff
      .unified_diff()
      .context_radius(PREVIEW_CONTEXT_RADIUS)
      .iter_hunks()
      .collect();
    let show_headers = hunks.len() > 1;

    for hunk in hunks {
      if show_headers {
        if full {
          omitted += 1;
        } else if lines.len() < MAX_PREVIEW_LINES {
          lines.push(ToolPreviewLine::header(hunk.header().to_string()));
        } else {
          full = true;
          omitted += 1;
        }
      }

      for change in hunk.iter_changes() {
        match change.tag() {
          ChangeTag::Insert => added += 1,
          ChangeTag::Delete => removed += 1,
          ChangeTag::Equal => {}
        }
        if full {
          omitted += 1;
          continue;
        }

        let (kind, marker, index) = match change.tag() {
          ChangeTag::Equal => {
            (ToolPreviewLineKind::Context, ' ', change.old_index())
          }
          ChangeTag::Delete => {
            (ToolPreviewLineKind::Removed, '-', change.old_index())
          }
          ChangeTag::Insert => {
            (ToolPreviewLineKind::Added, '+', change.new_index())
          }
        };
        let number = index.unwrap_or_default() + 1;
        let text = change.value().trim_end_matches(['\n', '\r']);
        let line = ToolPreviewLine::new(
          kind,
          format!("{number:>width$} {marker} {text}"),
        );
        if lines.len() < MAX_PREVIEW_LINES {
          lines.push(line);
        } else {
          full = true;
          omitted += 1;
        }
      }
    }

    if omitted > 0 {
      if lines.len() >= MAX_PREVIEW_LINES {
        lines.pop();
        omitted += 1;
      }
      let noun = if omitted == 1 { "line" } else { "lines" };
      lines.push(ToolPreviewLine::warning(format!("… {omitted} more {noun}")));
    }

    return ToolPreview::with_summary(lines, self.summary(added, removed));
  }

  fn summary(&self, added: usize, removed: usize) -> String {
    let range = if self.appended {
      let noun = if self.new_line_count == 1 {
        "line"
      } else {
        "lines"
      };
      format!("append {} {noun}", self.new_line_count)
    } else if self.start_line == self.end_line {
      format!("line {}", self.start_line)
    } else {
      format!("lines {}-{}", self.start_line, self.end_line)
    };
    let stat = match (added, removed) {
      (0, 0) => String::new(),
      (added, 0) => format!(" (+{added})"),
      (0, removed) => format!(" (-{removed})"),
      (added, removed) => format!(" (+{added} -{removed})"),
    };
    return format!("{range}{stat}");
  }
}

#[async_trait::async_trait]
impl Tool for EditFileTool {
  fn name(&self) -> &str {
    return EDIT_FILE_TOOL_NAME;
  }

  fn description(&self) -> &str {
    return "Edit a file by replacing a range of lines. ALWAYS first read the file with append_loc=true to get accurate line numbers. start_line and end_line are 1-based inclusive. If end_line exceeds the file length, new_content is appended after the last line. Use write_file to replace the whole file. Prefer many small edits over one large one.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "path": {"type": "string", "description": "Path to the file to edit"},
        "start_line": {"type": "integer", "description": "First line to replace, 1-based"},
        "end_line": {"type": "integer", "description": "Last line to replace, 1-based inclusive"},
        "new_content": {"type": "string", "description": "Content to replace the line range with"},
      },
      "required": ["path", "start_line", "end_line", "new_content"],
    });
  }

  fn is_read_only(&self) -> bool {
    return false;
  }

  async fn preview(&self, params: &ToolParams) -> Option<ToolPreview> {
    let plan = EditPlan::load(params).await.ok()?;
    return Some(plan.preview());
  }

  async fn execute(
    &self,
    params: ToolParams,
    _context: &mut ToolContext<'_>,
  ) -> Result<String> {
    let plan = EditPlan::load(&params).await?;

    write_file(&plan.path, &plan.new_file)
      .await
      .context(format!("Cannot write to file '{}'", plan.path))?;

    if plan.appended {
      let noun = if plan.new_line_count == 1 {
        "line"
      } else {
        "lines"
      };
      return Ok(format!(
        "Successfully appended {} {noun} to '{}'",
        plan.new_line_count, plan.path
      ));
    }

    return Ok(format!(
      "Successfully edited {} lines in '{}'",
      plan.replaced, plan.path
    ));
  }
}
