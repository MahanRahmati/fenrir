use std::sync::Arc;

use anyhow::{Context, Result, bail};
use chrono::Utc;
use uuid::Uuid;

use crate::config::Config;
use crate::persona_manager::COMPACTION_PERSONA;
use crate::prompt::Prompt;
use crate::provider_manager::ProviderManager;
use crate::session_manager::{Message, Summary};
use crate::vlog;

pub struct CompactionParams {
  pub messages: Vec<Message>,
  pub last_summary: Option<Summary>,
}

pub struct CompactionWorkflow;

impl CompactionWorkflow {
  pub async fn execute(
    provider: Arc<ProviderManager>,
    config: Config,
    params: CompactionParams,
  ) -> Result<String> {
    let prompt = Prompt::new(&config, &COMPACTION_PERSONA);
    let now = Utc::now().timestamp();
    let max_chars = config.compaction.tool_result_max_chars;

    let mut messages = Vec::new();
    messages.push(Message {
      id: Uuid::now_v7(),
      msg_type: None,
      role: "system".to_string(),
      content: prompt.get_system_prompt().to_string(),
      tool_call_id: None,
      tool_calls: None,
      timestamp: now,
    });

    let serialized: Vec<Message> = params
      .messages
      .iter()
      .filter_map(|msg| {
        let content = msg.serialize_for_compaction(max_chars);
        if content.is_empty() {
          return None;
        }
        Some(Message {
          id: msg.id,
          msg_type: msg.msg_type.clone(),
          role: msg.role.clone(),
          content,
          tool_call_id: msg.tool_call_id.clone(),
          tool_calls: msg.tool_calls.clone(),
          timestamp: msg.timestamp,
        })
      })
      .collect();

    if let Some(ref summary) = params.last_summary {
      let first_user_message = serialized.iter().find(|m| m.role == "user");
      if let Some(msg) = first_user_message {
        messages.push(msg.clone());
      }

      messages.push(Message {
        id: Uuid::now_v7(),
        msg_type: None,
        role: "system".to_string(),
        content: summary.content.clone(),
        tool_call_id: None,
        tool_calls: None,
        timestamp: summary.created_at,
      });

      for msg in &serialized {
        if msg.id > summary.covers_message_id {
          messages.push(msg.clone());
        }
      }
    } else {
      for msg in &serialized {
        messages.push(msg.clone());
      }
    }

    let response = provider
      .chat(&messages, &[])
      .await
      .context("Failed to execute compaction workflow")?;

    let Some(content) = response.content else {
      bail!("Compaction workflow returned no content");
    };

    let content = content.trim().to_string();
    if content.is_empty() {
      bail!("Compaction workflow returned empty content");
    }

    vlog!("Compacted session: {content}");
    return Ok(content);
  }
}
