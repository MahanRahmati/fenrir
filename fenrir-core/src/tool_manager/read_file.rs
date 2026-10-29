use anyhow::{Context, Result, bail};

use crate::files::operations::read_to_string;
use crate::tool_manager::cache::ToolCache;
use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};
use crate::utils::hash;
use crate::utils::path::resolve_path;

#[derive(Clone)]
struct ReadEntry {
  hash: u64,
  start: usize,
  end: usize,
}

pub struct ReadFileTool {
  cache: ToolCache<String, ReadEntry>,
}

impl ReadFileTool {
  pub fn new() -> Self {
    return Self {
      cache: ToolCache::new(),
    };
  }

  fn read_empty(&self, path: &str, hash: u64) -> Result<String> {
    let key = cache_key(path);
    if let Some(entry) = self.cache.get(&key)
      && entry.hash == hash
    {
      return Ok("File unchanged since last read.".to_string());
    }
    self.cache.insert(
      key,
      ReadEntry {
        hash,
        start: 0,
        end: 0,
      },
    );
    return Ok("".to_string());
  }
}

#[async_trait::async_trait]
impl Tool for ReadFileTool {
  fn name(&self) -> &str {
    return "read_file";
  }

  fn description(&self) -> &str {
    return "Read the contents of a file. Optionally specify a 1-based line range. If append_loc is true, each line is prefixed with its line number (e.g. \"1\u{2192} ...\"). Re-reading an unchanged file with the same path and line range returns a short confirmation instead of the content.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "path": {"type": "string", "description": "Path to the file"},
        "start_line": {"type": "integer", "description": "First line to read, 1-based (default: 1)"},
        "end_line": {"type": "integer", "description": "Last line to read, 1-based inclusive (default: end of file)"},
        "append_loc": {"type": "boolean", "description": "Prefix each line with its line number"},
      },
      "required": ["path"],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  fn reset_cache(&self) {
    self.cache.clear();
  }

  async fn execute(
    &self,
    params: ToolParams,
    _context: &mut ToolContext<'_>,
  ) -> Result<String> {
    let path = get_param::<String>(&params, "read_file", "path")?;

    let content = read_to_string(&path)
      .await
      .context(format!("Cannot read file '{path}'"))?;

    let hash = hash::file_hash(&content);
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    let start_line: usize =
      get_optional_param::<usize>(&params, "read_file", "start_line")
        .unwrap_or(1)
        .saturating_sub(1);

    let end_line: Option<usize> =
      get_optional_param::<usize>(&params, "read_file", "end_line")
        .map(|v: usize| v.saturating_sub(1));

    if total_lines == 0 {
      return self.read_empty(&path, hash);
    }

    let start = start_line.min(total_lines - 1);
    let end = end_line
      .map(|v| v.min(total_lines - 1))
      .unwrap_or(total_lines - 1);

    if start > end {
      bail!(
        "start_line ({}) is greater than end_line ({})",
        start + 1,
        end + 1
      );
    }

    let key = cache_key(&path);
    if let Some(entry) = self.cache.get(&key)
      && entry.hash == hash
      && entry.start == start + 1
      && entry.end == end + 1
    {
      return Ok("File unchanged since last read.".to_string());
    }

    let selected_lines = &lines[start..=end];
    let append_loc: bool =
      get_optional_param::<bool>(&params, "read_file", "append_loc")
        .unwrap_or(false);

    let output: String = if append_loc {
      selected_lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{}{} {}", start + i + 1, '\u{2192}', line))
        .collect::<Vec<String>>()
        .join("\n")
    } else {
      selected_lines.join("\n")
    };

    self.cache.insert(
      key,
      ReadEntry {
        hash,
        start: start + 1,
        end: end + 1,
      },
    );

    return Ok(output);
  }
}

fn cache_key(path: &str) -> String {
  let cwd = std::env::current_dir().unwrap_or_default();
  return resolve_path(path, &cwd).to_string_lossy().to_string();
}
