use std::collections::HashMap;

use anyhow::{Context, Result};
use futures::{StreamExt, future};

use crate::config::ProviderModel;
use crate::network::HttpClient;
use crate::provider_manager::provider::{
  ChatResponse, ChatStream, Model, Provider,
};
use crate::provider_manager::providers::openai_compatible_api::{
  ChatCompletionRequest, ChatCompletionResponse, ExtraBody, ExtraMetadata,
  ModelsResponse, build_headers, build_request, parse_context_window,
  parse_response, parse_sse_chunk,
};
use crate::session_manager::Message;
use crate::tool_manager::tool::ToolSpec;
use crate::vlog;

pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const OPENROUTER_PROVIDER_NAME: &str = "open_router";

#[derive(Clone)]
pub struct OpenRouterProvider {
  name: String,
  api_key: Option<String>,
  models: Vec<Model>,
  client: HttpClient,
}

impl OpenRouterProvider {
  pub fn new(api_key: Option<String>, models: Vec<ProviderModel>) -> Self {
    let models: Vec<Model> = models
      .into_iter()
      .map(|m| Model {
        provider: OPENROUTER_PROVIDER_NAME.to_string(),
        name: m.name,
        alias: m.alias,
      })
      .collect();
    let client = HttpClient::new(OPENROUTER_BASE_URL.to_string());
    return Self {
      name: OPENROUTER_PROVIDER_NAME.to_string(),
      api_key,
      models,
      client,
    };
  }

  fn build_headers(&self) -> Option<HashMap<String, String>> {
    let mut headers = build_headers(self.api_key.as_deref());
    headers.insert("X-Title".to_string(), "Fenrir".to_string());
    return Some(headers);
  }

  async fn build_request(
    &self,
    model: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    stream: bool,
  ) -> Result<ChatCompletionRequest> {
    return build_request(
      model,
      messages,
      tools,
      stream,
      Some(ExtraBody {
        metadata: Some(ExtraMetadata {
          source: "fenrir".to_string(),
        }),
      }),
    )
    .await;
  }
}

#[async_trait::async_trait]
impl Provider for OpenRouterProvider {
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
  ) -> Result<ChatResponse> {
    let request = self.build_request(model, messages, tools, false).await?;
    let headers = self.build_headers();

    vlog!("Sending chat request to OpenRouter");

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
  ) -> Result<ChatStream> {
    let request = self.build_request(model, messages, tools, true).await?;
    let headers = self.build_headers();

    vlog!("Sending streaming chat request to OpenRouter");

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
