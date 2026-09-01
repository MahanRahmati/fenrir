use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::get_param;
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;

pub struct DdgTool;

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

  async fn execute(&self, params: ToolParams) -> ToolManagerResult<String> {
    let keyword = get_param::<String>(&params, "ddg", "keyword")?;

    let output = process::run("ddgr", &["-x", "--json", keyword.as_str()])
      .await
      .map_err(|e| ToolManagerError::ExecutionFailed {
        name: "ddg".to_string(),
        reason: e,
      })?;

    let results: Vec<serde_json::Value> = serde_json::from_str(&output.stdout)
      .map_err(|e| ToolManagerError::ExecutionFailed {
        name: "ddg".to_string(),
        reason: format!("Failed to parse ddgr JSON output: {}", e),
      })?;

    if results.is_empty() {
      return Ok(format!("No results found for '{}' .", keyword));
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
      formatted.push_str(&format!(
        "{}. {} ({})\n{}\n\n",
        i + 1,
        title,
        url,
        abstract_text
      ));
    }

    return Ok(formatted.trim_end_matches('\n').to_string());
  }
}
