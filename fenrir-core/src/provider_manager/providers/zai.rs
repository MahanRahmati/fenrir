use std::collections::HashMap;

use anyhow::{Context, Result};
use futures::{StreamExt, future};

use crate::config::ProviderModel;
use crate::network::{HttpClient, NetworkTimeouts};
use crate::provider_manager::provider::{
  ChatResponse, ChatStream, Model, Provider,
};
use crate::provider_manager::providers::openai_compatible_api::{
  ChatCompletionRequest, ChatCompletionResponse, ModelsResponse, build_headers,
  build_request, parse_context_window, parse_response, parse_sse_chunk,
};
use crate::session_manager::Message;
use crate::skill_manager::SkillRegistry;
use crate::tool_manager::tool::ToolSpec;
use crate::vlog;

pub const ZAI_BASE_URL: &str = "https://api.z.ai/api/paas/v4";
pub const ZAI_PROVIDER_NAME: &str = "zai";

#[derive(Clone)]
pub struct ZaiProvider {
  name: String,
  api_key: Option<String>,
  models: Vec<Model>,
  client: HttpClient,
}

impl ZaiProvider {
  pub fn new(
    api_key: Option<String>,
    models: Vec<ProviderModel>,
    timeouts: NetworkTimeouts,
  ) -> Self {
    let models: Vec<Model> = models
      .into_iter()
      .map(|m| Model {
        provider: ZAI_PROVIDER_NAME.to_string(),
        name: m.name,
        alias: m.alias,
      })
      .collect();
    let client = HttpClient::new(ZAI_BASE_URL.to_string(), timeouts);
    return Self {
      name: ZAI_PROVIDER_NAME.to_string(),
      api_key,
      models,
      client,
    };
  }

  fn build_headers(&self) -> Option<HashMap<String, String>> {
    return Some(build_headers(self.api_key.as_deref()));
  }

  async fn build_request(
    &self,
    model: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    stream: bool,
    skills: Option<&SkillRegistry>,
  ) -> Result<ChatCompletionRequest> {
    return build_request(model, messages, tools, stream, None, skills).await;
  }
}

#[async_trait::async_trait]
impl Provider for ZaiProvider {
  fn name(&self) -> &str {
    return &self.name;
  }

  fn list_models(&self) -> Vec<Model> {
    return self.models.clone();
  }

  async fn context_window(&self, model: &str) -> Result<Option<u64>> {
    let headers = self.build_headers();
    let response: ModelsResponse = self
      .client
      .get("/models", headers)
      .await
      .context("Failed to fetch models")?;
    return Ok(parse_context_window(&response, model));
  }

  async fn chat(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
    model: &str,
    skills: Option<&SkillRegistry>,
  ) -> Result<ChatResponse> {
    let request = self
      .build_request(model, messages, tools, false, skills)
      .await?;
    let headers = self.build_headers();

    vlog!("Sending chat request to z.ai");

    let response: ChatCompletionResponse = self
      .client
      .post(&request, "/chat/completions", headers)
      .await
      .context("Failed to send chat request")?;

    return parse_response(&response);
  }

  async fn chat_stream(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
    model: &str,
    skills: Option<&SkillRegistry>,
  ) -> Result<ChatStream> {
    let request = self
      .build_request(model, messages, tools, true, skills)
      .await?;
    let headers = self.build_headers();

    vlog!("Sending streaming chat request to z.ai");

    let sse_stream = self
      .client
      .post_stream(&request, "/chat/completions", headers)
      .await
      .context("Failed to start chat stream")?;

    let mapped = sse_stream.filter_map(|chunk_result| {
      future::ready(match chunk_result {
        Ok(chunk) => parse_sse_chunk(chunk),
        Err(e) => Some(Err(e).context("Failed to read chat stream")),
      })
    });

    return Ok(Box::pin(mapped));
  }
}
