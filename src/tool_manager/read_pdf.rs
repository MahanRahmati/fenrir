use crate::capability::pdf;
use crate::tool_manager::errors::{ToolManagerError, ToolManagerResult};
use crate::tool_manager::helpers::*;
use crate::tool_manager::tool::{Tool, ToolParams};

pub struct ReadPdfTool;

#[async_trait::async_trait]
impl Tool for ReadPdfTool {
  fn name(&self) -> &str {
    return "read_pdf";
  }

  fn description(&self) -> &str {
    return "Render a single page of a PDF as an image and attach it to the conversation. Use this when a PDF mentioned with @path has not had its pages attached — pass the same path and a 1-based page number.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "path": {"type": "string", "description": "Path to the PDF file"},
        "page": {"type": "integer", "description": "1-based page number to read"},
      },
      "required": ["path", "page"],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  async fn execute(&self, params: ToolParams) -> ToolManagerResult<String> {
    let path = get_param::<String>(&params, "read_pdf", "path")?;
    let path = path.trim_start_matches('@').to_string();
    let page = get_param::<u32>(&params, "read_pdf", "page")?;
    if page == 0 {
      return Err(ToolManagerError::ExecutionFailed {
        name: "read_pdf".to_string(),
        reason: "Page numbers are 1-based".to_string(),
      });
    }

    let info = pdf::pdf_info(&path).await.map_err(|e| {
      ToolManagerError::ExecutionFailed {
        name: "read_pdf".to_string(),
        reason: e.to_string(),
      }
    })?;
    if info.pages == 0 || page > info.pages {
      return Err(ToolManagerError::ExecutionFailed {
        name: "read_pdf".to_string(),
        reason: format!(
          "Page {page} is out of range: '{path}' has {} page(s)",
          info.pages
        ),
      });
    }

    let image = pdf::render_page(&path, page).await.map_err(|e| {
      ToolManagerError::ExecutionFailed {
        name: "read_pdf".to_string(),
        reason: e.to_string(),
      }
    })?;

    return Ok(format!("@{image} (Page {page} of {})", info.pages));
  }
}
