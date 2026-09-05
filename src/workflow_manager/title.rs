use std::sync::Arc;

use uuid::Uuid;

use crate::config::Config;
use crate::persona_manager::TITLES_PERSONA;
use crate::prompt::Prompt;
use crate::provider_manager::ProviderManager;
use crate::session_manager::Message;
use crate::vlog;
use crate::workflow_manager::errors::{
  WorkflowManagerError, WorkflowManagerResult,
};
use crate::workflow_manager::workflow::{Workflow, WorkflowParams};

pub struct TitleWorkflow;

#[async_trait::async_trait]
impl Workflow for TitleWorkflow {
  fn name(&self) -> &str {
    return "title_generator";
  }

  async fn execute(
    &self,
    provider: Arc<ProviderManager>,
    config: Config,
    params: WorkflowParams,
  ) -> WorkflowManagerResult<String> {
    let message = match params.get("message") {
      Some(m) if !m.is_empty() => m.clone(),
      _ => return Ok(String::new()),
    };

    let prompt = Prompt::new(&config, &TITLES_PERSONA);
    let now = chrono::Utc::now().timestamp();
    let messages = vec![
      Message {
        id: Uuid::now_v7(),
        msg_type: None,
        role: "system".to_string(),
        content: prompt.get_system_prompt().to_string(),
        tool_call_id: None,
        tool_calls: None,
        timestamp: now,
      },
      Message {
        id: Uuid::now_v7(),
        msg_type: None,
        role: "user".to_string(),
        content: message,
        tool_call_id: None,
        tool_calls: None,
        timestamp: now,
      },
    ];

    let response = match provider.chat(&messages, &[]).await {
      Ok(response) => response,
      Err(e) => {
        vlog!("Failed to generate session title: {e}");
        return Err(WorkflowManagerError::ExecutionFailed {
          name: "title".to_string(),
          reason: e.to_string(),
        });
      }
    };

    let Some(content) = response.content else {
      return Ok(String::new());
    };

    let title = content
      .trim()
      .trim_matches(|c| c == '"' || c == '\'' || c == '`')
      .to_string();

    if title.is_empty() {
      return Ok(String::new());
    }

    vlog!("Generated session title: {title}");
    return Ok(title);
  }
}
