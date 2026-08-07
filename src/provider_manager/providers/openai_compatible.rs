use std::collections::HashMap;

use futures::{StreamExt, future};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::ProviderModel;
use crate::files::operations::{read_bytes, read_to_string};
use crate::mentions::{PathKind, classify_path, extract_paths, mime_type};
use crate::network::HttpClient;
use crate::network::sse::SSEChunk;
use crate::provider_manager::errors::{
  ProviderManagerError, ProviderManagerResult,
};
use crate::provider_manager::provider::{
  ChatResponse, ChatStream, ChatStreamDelta, ChatUsage, Model, Provider,
  StreamToolCallDelta, ToolCall,
};
use crate::session_manager::Message;
use crate::tool_manager::tool::ToolSpec;
use crate::vlog;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

#[derive(Debug, Serialize, Deserialize)]
struct ChatCompletionRequest {
  model: String,
  messages: Vec<ChatMessage>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  tools: Vec<ChatTool>,
  stream: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  stream_options: Option<StreamOptions>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StreamOptions {
  include_usage: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
  role: String,
  content: Option<ChatContent>,
  #[serde(skip_serializing_if = "Option::is_none")]
  tool_calls: Option<Vec<ChatToolCall>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ChatContent {
  Text(String),
  Parts(Vec<ContentPart>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart {
  Text { text: String },
  ImageUrl { image_url: ImageUrlPart },
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageUrlPart {
  url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatToolCall {
  id: String,
  #[serde(rename = "type")]
  call_type: String,
  function: ChatToolFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatToolFunction {
  name: String,
  arguments: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatTool {
  #[serde(rename = "type")]
  tool_type: String,
  function: ToolFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolFunction {
  name: String,
  description: String,
  parameters: Value,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
  choices: Vec<ResponseChoice>,
  usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ResponseChoice {
  message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
  content: Option<String>,
  reasoning_content: Option<String>,
  #[serde(default)]
  tool_calls: Vec<ApiResponseToolCall>,
}

#[derive(Debug, Deserialize)]
struct ApiResponseToolCall {
  id: String,
  function: ApiFunctionCall,
}

#[derive(Debug, Deserialize)]
struct ApiFunctionCall {
  name: String,
  arguments: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionStreamResponse {
  choices: Vec<StreamChoice>,
  usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
  delta: StreamDelta,
}

#[derive(Debug, Default, Deserialize)]
struct StreamDelta {
  content: Option<String>,
  reasoning_content: Option<String>,
  #[serde(default)]
  tool_calls: Vec<StreamToolCall>,
}

#[derive(Debug, Deserialize)]
struct StreamToolCall {
  index: usize,
  id: Option<String>,
  function: Option<StreamFunction>,
}

#[derive(Debug, Deserialize)]
struct StreamFunction {
  name: Option<String>,
  arguments: Option<String>,
}

#[derive(Clone)]
pub struct OpenAICompatibleProvider {
  name: String,
  api_key: Option<String>,
  models: Vec<Model>,
  active_model: Option<Model>,
  client: HttpClient,
}

impl OpenAICompatibleProvider {
  pub fn new(
    url: String,
    name: String,
    api_key: Option<String>,
    models: Vec<ProviderModel>,
  ) -> Self {
    let models: Vec<Model> = models
      .into_iter()
      .map(|m| Model {
        name: m.name,
        alias: m.alias,
      })
      .collect();
    let client = HttpClient::new(url);
    return Self {
      name,
      api_key,
      models,
      active_model: None,
      client,
    };
  }

  fn build_headers(&self) -> Option<HashMap<String, String>> {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    let api_key = self.api_key.clone();
    if let Some(api_key) = api_key {
      headers.insert("Authorization".to_string(), format!("Bearer {api_key}"));
    }
    return Some(headers);
  }

  async fn build_request(
    &self,
    model: String,
    messages: &[Message],
    tools: &[ToolSpec],
    stream: bool,
  ) -> ProviderManagerResult<ChatCompletionRequest> {
    let mut chat_messages: Vec<ChatMessage> = Vec::new();
    for msg in messages {
      if msg.role == "reasoning" {
        continue;
      }

      let content = if msg.content.is_empty() {
        None
      } else if msg.role != "user" {
        Some(ChatContent::Text(msg.content.clone()))
      } else {
        let paths = extract_paths(&msg.content);
        if paths.is_empty() {
          Some(ChatContent::Text(msg.content.clone()))
        } else {
          let mut parts = vec![ContentPart::Text {
            text: msg.content.clone(),
          }];
          for path in paths {
            let kind = classify_path(&path).ok_or_else(|| {
              ProviderManagerError::ProviderError(format!(
                "Cannot attach '{path}': unsupported file (no extension)"
              ))
            })?;
            match kind {
              PathKind::Image => {
                let bytes = read_bytes(&path).await.map_err(|e| {
                  ProviderManagerError::ProviderError(format!(
                    "Cannot attach '@{path}': {e}"
                  ))
                })?;
                let mime = mime_type(&path).ok_or_else(|| {
                  ProviderManagerError::ProviderError(format!(
                    "Cannot attach '@{path}': unsupported image type"
                  ))
                })?;
                let encoded = STANDARD.encode(&bytes);
                parts.push(ContentPart::ImageUrl {
                  image_url: ImageUrlPart {
                    url: format!("data:{mime};base64,{encoded}"),
                  },
                });
              }
              PathKind::Text => {
                let contents = read_to_string(&path).await.map_err(|e| {
                  ProviderManagerError::ProviderError(format!(
                    "Cannot attach '@{path}': {e}"
                  ))
                })?;
                parts.push(ContentPart::Text {
                  text: format!("@{path}:\n{contents}"),
                });
              }
            }
          }
          Some(ChatContent::Parts(parts))
        }
      };

      let tool_calls = msg.tool_calls.as_ref().map(|calls| {
        calls
          .iter()
          .map(|tc| ChatToolCall {
            id: tc.id.clone(),
            call_type: tc.call_type.clone(),
            function: ChatToolFunction {
              name: tc.function.name.clone(),
              arguments: tc.function.arguments.clone(),
            },
          })
          .collect()
      });

      chat_messages.push(ChatMessage {
        role: msg.role.clone(),
        content,
        tool_calls,
        tool_call_id: msg.tool_call_id.clone(),
      });
    }

    let chat_tools: Vec<ChatTool> = tools
      .iter()
      .map(|t| ChatTool {
        tool_type: "function".to_string(),
        function: ToolFunction {
          name: t.name.clone(),
          description: t.description.clone(),
          parameters: t.parameters.clone(),
        },
      })
      .collect();

    let request = ChatCompletionRequest {
      model,
      messages: chat_messages,
      tools: chat_tools,
      stream,
      stream_options: if stream {
        Some(StreamOptions {
          include_usage: true,
        })
      } else {
        None
      },
    };

    return Ok(request);
  }
}

fn parse_sse_chunk(
  chunk: SSEChunk,
) -> Option<ProviderManagerResult<ChatStreamDelta>> {
  if chunk.event == "error" {
    return Some(Err(ProviderManagerError::ProviderError(chunk.data)));
  }
  if chunk.data.trim() == "[DONE]" {
    return None;
  }
  match serde_json::from_str::<ChatCompletionStreamResponse>(&chunk.data) {
    Ok(stream_resp) => {
      if stream_resp.choices.is_empty()
        && let Some(usage) = stream_resp.usage
      {
        return Some(Ok(ChatStreamDelta {
          content: None,
          reasoning: None,
          tool_calls: Vec::new(),
          usage: Some(usage),
        }));
      }
      let choice = stream_resp.choices.into_iter().next()?;
      let delta = choice.delta;
      let tool_calls = delta
        .tool_calls
        .into_iter()
        .map(|tc| StreamToolCallDelta {
          index: tc.index,
          id: tc.id,
          name: tc.function.as_ref().and_then(|f| f.name.clone()),
          arguments: tc.function.map(|f| f.arguments.unwrap_or_default()),
        })
        .collect();
      return Some(Ok(ChatStreamDelta {
        content: delta.content,
        reasoning: delta.reasoning_content,
        tool_calls,
        usage: None,
      }));
    }
    Err(e) => {
      return Some(Err(ProviderManagerError::ProviderError(e.to_string())));
    }
  }
}

#[async_trait::async_trait]
impl Provider for OpenAICompatibleProvider {
  fn name(&self) -> &str {
    return &self.name;
  }

  fn list_models(&self) -> Vec<Model> {
    return self
      .models
      .iter()
      .map(|m| Model {
        name: m.name.clone(),
        alias: m.alias.clone(),
      })
      .collect();
  }

  fn set_active_model(&mut self, model: Model) {
    self.active_model = Some(model);
  }

  async fn chat(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> ProviderManagerResult<ChatResponse> {
    let model = self
      .active_model
      .as_ref()
      .ok_or_else(|| ProviderManagerError::NoActiveModel)?
      .name
      .clone();
    let request = self.build_request(model, messages, tools, false).await?;
    let headers = self.build_headers();

    vlog!("Sending chat request to open ai compatible server");

    let response: ChatCompletionResponse = self
      .client
      .post(&request, "/v1/chat/completions", headers)
      .await
      .map_err(|e| ProviderManagerError::ProviderError(e.to_string()))?;

    if response.choices.is_empty() {
      return Err(ProviderManagerError::EmptyResponseChoices);
    }

    let choice = &response.choices[0];
    let content = choice.message.content.clone();
    let reasoning = choice.message.reasoning_content.clone();
    let tool_calls: Vec<ToolCall> = choice
      .message
      .tool_calls
      .iter()
      .map(|tc| ToolCall {
        id: tc.id.clone(),
        name: tc.function.name.clone(),
        arguments: tc.function.arguments.clone(),
      })
      .collect();

    for tc in &tool_calls {
      vlog!(
        "LLM tool call: {} with arguments: {}",
        tc.name,
        tc.arguments
      );
    }
    vlog!(
      "Chat response: content={}, tool_calls={}",
      content.as_deref().unwrap_or("none"),
      tool_calls.len()
    );

    return Ok(ChatResponse {
      content,
      reasoning,
      tool_calls,
      usage: response.usage,
    });
  }

  async fn chat_stream(
    &self,
    messages: &[Message],
    tools: &[ToolSpec],
  ) -> ProviderManagerResult<ChatStream> {
    let model = self
      .active_model
      .as_ref()
      .ok_or_else(|| ProviderManagerError::NoActiveModel)?
      .name
      .clone();
    let request = self.build_request(model, messages, tools, true).await?;
    let headers = self.build_headers();

    vlog!("Sending streaming chat request to open ai compatible server");

    let sse_stream = self
      .client
      .post_stream(&request, "/v1/chat/completions", headers)
      .await
      .map_err(|e| ProviderManagerError::ProviderError(e.to_string()))?;

    let mapped = sse_stream.filter_map(|chunk_result| {
      future::ready(match chunk_result {
        Ok(chunk) => parse_sse_chunk(chunk),
        Err(e) => Some(Err(ProviderManagerError::ProviderError(e.to_string()))),
      })
    });

    return Ok(Box::pin(mapped));
  }
}
