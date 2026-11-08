use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::pdf;
use crate::files::operations::{read_bytes, read_to_string};
use crate::mentions::{
  PathKind, classify_path, extract_paths, mime_type, strip_mentions,
};
use crate::network::sse::SSEChunk;
use crate::provider_manager::provider::{
  ChatResponse, ChatStreamDelta, ChatUsage, StreamToolCallDelta, ToolCall,
};
use crate::session_manager::Message;
use crate::tool_manager::tool::ToolSpec;
use crate::vlog;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
  pub model: String,
  pub messages: Vec<ChatMessage>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub tools: Vec<ChatTool>,
  pub stream: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stream_options: Option<StreamOptions>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub extra_body: Option<ExtraBody>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtraBody {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub metadata: Option<ExtraMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtraMetadata {
  pub source: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamOptions {
  pub include_usage: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
  pub role: String,
  pub content: Option<ChatContent>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tool_calls: Option<Vec<ChatToolCall>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatContent {
  Text(String),
  Parts(Vec<ContentPart>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
  Text { text: String },
  ImageUrl { image_url: ImageUrlPart },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageUrlPart {
  pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatToolCall {
  pub id: String,
  #[serde(rename = "type")]
  pub call_type: String,
  pub function: ChatToolFunction,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatToolFunction {
  pub name: String,
  pub arguments: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatTool {
  #[serde(rename = "type")]
  pub tool_type: String,
  pub function: ToolFunction,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolFunction {
  pub name: String,
  pub description: String,
  pub parameters: Value,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
  pub choices: Vec<ResponseChoice>,
  pub usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseChoice {
  pub message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
  pub content: Option<String>,
  #[serde(alias = "reasoning")]
  pub reasoning_content: Option<String>,
  #[serde(default)]
  pub tool_calls: Vec<ApiResponseToolCall>,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponseToolCall {
  pub id: String,
  pub function: ApiFunctionCall,
}

#[derive(Debug, Deserialize)]
pub struct ApiFunctionCall {
  pub name: String,
  pub arguments: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionStreamResponse {
  pub choices: Vec<StreamChoice>,
  pub usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
pub struct StreamChoice {
  pub delta: StreamDelta,
}

#[derive(Debug, Default, Deserialize)]
pub struct StreamDelta {
  pub content: Option<String>,
  #[serde(alias = "reasoning")]
  pub reasoning_content: Option<String>,
  #[serde(default)]
  pub tool_calls: Vec<StreamToolCall>,
}

#[derive(Debug, Deserialize)]
pub struct StreamToolCall {
  pub index: usize,
  pub id: Option<String>,
  pub function: Option<StreamFunction>,
}

#[derive(Debug, Deserialize)]
pub struct StreamFunction {
  pub name: Option<String>,
  pub arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModelsResponse {
  pub data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ModelEntry {
  pub id: String,
  #[serde(default)]
  pub meta: Option<ModelMeta>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelMeta {
  pub n_ctx: Option<u64>,
}

pub fn build_headers(api_key: Option<&str>) -> HashMap<String, String> {
  let mut headers = HashMap::new();
  headers.insert("Content-Type".to_string(), "application/json".to_string());
  if let Some(api_key) = api_key {
    headers.insert("Authorization".to_string(), format!("Bearer {api_key}"));
  }
  return headers;
}

pub fn build_chat_tools(tools: &[ToolSpec]) -> Vec<ChatTool> {
  tools
    .iter()
    .map(|t| ChatTool {
      tool_type: "function".to_string(),
      function: ToolFunction {
        name: t.name.clone(),
        description: t.description.clone(),
        parameters: t.parameters.clone(),
      },
    })
    .collect()
}

pub async fn build_messages(messages: &[Message]) -> Result<Vec<ChatMessage>> {
  let mut chat_messages: Vec<ChatMessage> = Vec::new();
  for msg in messages {
    if msg.role == "reasoning" {
      continue;
    }

    let content = if msg.content.is_empty() {
      None
    } else if msg.role != "user" && msg.role != "tool" {
      Some(ChatContent::Text(msg.content.clone()))
    } else {
      Some(build_content_parts(&msg.content, msg.role == "user").await?)
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
  return Ok(chat_messages);
}

pub async fn build_request(
  model: &str,
  messages: &[Message],
  tools: &[ToolSpec],
  stream: bool,
  extra_body: Option<ExtraBody>,
) -> Result<ChatCompletionRequest> {
  let chat_messages = build_messages(messages).await?;
  let chat_tools = build_chat_tools(tools);
  return Ok(ChatCompletionRequest {
    model: model.to_string(),
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
    extra_body,
  });
}

pub fn parse_response(
  response: &ChatCompletionResponse,
) -> Result<ChatResponse> {
  if response.choices.is_empty() {
    bail!("Empty response choices");
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

pub fn parse_sse_chunk(chunk: SSEChunk) -> Option<Result<ChatStreamDelta>> {
  if chunk.event == "error" {
    return Some(Err(anyhow!("Failed to parse sse '{}'", chunk.data)));
  }
  let data = chunk.data.trim();
  if data.is_empty() || data == "[DONE]" {
    return None;
  }
  match serde_json::from_str::<ChatCompletionStreamResponse>(data) {
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
      let content = delta.content.filter(|text| !text.is_empty());
      let reasoning = delta.reasoning_content.filter(|text| !text.is_empty());
      return Some(Ok(ChatStreamDelta {
        content,
        reasoning,
        tool_calls,
        usage: None,
      }));
    }
    Err(e) => {
      return Some(Err(e).context("Failed to parse stream chunk"));
    }
  }
}

async fn build_content_parts(
  content: &str,
  strict: bool,
) -> Result<ChatContent> {
  let paths = extract_paths(content);
  if paths.is_empty() {
    return Ok(ChatContent::Text(content.to_string()));
  }
  let text = strip_mentions(content);
  let mut parts = Vec::new();
  if !text.trim().is_empty() {
    parts.push(ContentPart::Text { text });
  }
  for path in paths {
    let kind = match classify_path(&path) {
      Some(kind) => kind,
      None => {
        if strict {
          bail!("Cannot attach '{path}': unsupported file (no extension)");
        }
        continue;
      }
    };
    match kind {
      PathKind::Image => {
        let bytes = match read_bytes(&path).await {
          Ok(bytes) => bytes,
          Err(e) => {
            if strict {
              return Err(e).context(format!("Cannot attach '@{path}'"));
            }
            continue;
          }
        };
        let Some(mime) = mime_type(&path) else {
          if strict {
            bail!("Cannot attach '@{path}': unsupported image type");
          }
          continue;
        };
        let encoded = STANDARD.encode(&bytes);
        parts.push(ContentPart::Text {
          text: format!("@{path} (attached as image below)"),
        });
        parts.push(ContentPart::ImageUrl {
          image_url: ImageUrlPart {
            url: format!("data:{mime};base64,{encoded}"),
          },
        });
      }
      PathKind::Text => {
        let contents = match read_to_string(&path).await {
          Ok(contents) => contents,
          Err(e) => {
            if strict {
              return Err(e).context(format!("Cannot attach '@{path}'"));
            }
            continue;
          }
        };
        parts.push(ContentPart::Text {
          text: format!("@{path}:\n{contents}"),
        });
      }
      PathKind::Pdf => {
        let info = match pdf::pdf_info(&path).await {
          Ok(info) => info,
          Err(e) => {
            if strict {
              return Err(e).context(format!("Cannot inspect '@{path}'"));
            }
            continue;
          }
        };
        parts.push(ContentPart::Text {
          text: format!(
            "@{path} ({} pages, {}). Pages are not attached — use the read_pdf tool with the path (without the @ symbol) and a 1-based page number to view any page as an image.",
            info.pages, info.file_size
          ),
        });
      }
    }
  }
  return Ok(ChatContent::Parts(parts));
}

pub fn parse_context_window(
  response: &ModelsResponse,
  model: &str,
) -> Option<u64> {
  response
    .data
    .iter()
    .find(|entry| entry.id == model)
    .and_then(|entry| entry.meta.clone())
    .and_then(|meta| meta.n_ctx)
    .filter(|n_ctx| *n_ctx > 0)
}
