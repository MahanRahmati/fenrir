use anyhow::{Context, Result};

use crate::tool_manager::cache::ToolCache;
use crate::tool_manager::helpers::get_param;
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;

pub struct DdgTool {
  cache: ToolCache<String, String>,
}

impl DdgTool {
  pub fn new() -> Self {
    return Self {
      cache: ToolCache::new(),
    };
  }
}

#[async_trait::async_trait]
impl Tool for DdgTool {
  fn name(&self) -> &str {
    return "ddg";
  }

  fn description(&self) -> &str {
    return "Search the web using DuckDuckGo via ddgr.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "keyword": {"type": "string", "description": "Search keywords"}
      },
      "required": ["keyword"],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  fn reset_cache(&self) {
    self.cache.clear();
  }

  async fn execute(&self, params: ToolParams) -> Result<String> {
    let keyword = get_param::<String>(&params, "ddg", "keyword")?;

    if let Some(cached) = self.cache.get(&keyword) {
      return Ok(cached);
    }

    let output = process::run("ddgr", &["-x", "--json", keyword.as_str()])
      .await
      .context("Failed to run ddgr")?;

    let results: Vec<serde_json::Value> = serde_json::from_str(&output.stdout)
      .context("Failed to parse ddgr JSON output")?;

    if results.is_empty() {
      return Ok(format!("No results found for '{keyword}' ."));
    }

    let mut formatted = String::new();
    for (i, result) in results.iter().enumerate() {
      let title = result
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(no title)");
      let url = result
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("(no url)");
      let abstract_text = result
        .get("abstract")
        .and_then(|v| v.as_str())
        .unwrap_or("(no abstract)");
      formatted
        .push_str(&format!("{}. {title} ({url})\n{abstract_text}\n\n", i + 1,));
    }

    let result = formatted.trim_end_matches('\n').to_string();
    self.cache.insert(keyword, result.clone());
    return Ok(result);
  }
}
