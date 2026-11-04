use anyhow::Result;

use crate::provider_manager::ProviderManager;
use crate::session_manager::Message;
use crate::session_manager::session::Session;
use crate::vlog;
use crate::workflow_manager::prediction::PredictionParams;

const MAX_CONTEXT_MESSAGES: usize = 10;

impl Session {
  pub async fn predict_next_prompt(
    &self,
    providers: &ProviderManager,
  ) -> Result<String> {
    if self.is_subagent() {
      return Ok(String::new());
    }
    let Some(model) = self.model.as_ref() else {
      return Ok(String::new());
    };

    let params = PredictionParams {
      messages: self.prediction_messages(),
      summary: self.summaries.last().map(|summary| summary.content.clone()),
    };
    if params.messages.is_empty() {
      return Ok(String::new());
    }

    let prediction = match self
      .workflow_manager
      .predict_next_prompt(providers, model, params)
      .await
    {
      Ok(prediction) => prediction,
      Err(e) => {
        vlog!("Failed to predict next prompt: {e}");
        return Ok(String::new());
      }
    };

    return Ok(prediction);
  }

  fn prediction_messages(&self) -> Vec<Message> {
    let mut messages: Vec<Message> = self
      .build_chat_context()
      .into_iter()
      .filter(|message| {
        (message.role == "user" || message.role == "assistant")
          && message.tool_calls.is_none()
          && !message.content.trim().is_empty()
      })
      .collect();
    if messages.len() > MAX_CONTEXT_MESSAGES {
      messages = messages.split_off(messages.len() - MAX_CONTEXT_MESSAGES);
    }
    return messages;
  }
}
