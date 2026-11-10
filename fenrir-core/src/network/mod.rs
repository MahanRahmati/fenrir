pub mod error;
pub mod retry;
pub mod sse;

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::header::HeaderMap;
use reqwest::{Client, RequestBuilder, Response};
use serde::Serialize;
use tokio::time;

use crate::config::NetworkConfig;
use crate::network::error::NetworkError;
use crate::network::sse::SSEStream;
use crate::vlog;

#[derive(Debug, Clone, Copy)]
pub struct NetworkTimeouts {
  pub connect: Duration,
  pub request: Duration,
}

impl NetworkTimeouts {
  pub fn from_config(config: &NetworkConfig) -> Self {
    return Self {
      connect: Duration::from_secs(config.connect_timeout_seconds),
      request: Duration::from_secs(config.request_timeout_seconds),
    };
  }
}

#[derive(Debug, Clone)]
pub struct HttpClient {
  base_url: String,
  client: Client,
  timeouts: NetworkTimeouts,
}

impl HttpClient {
  pub fn new(base_url: String, timeouts: NetworkTimeouts) -> Self {
    let mut builder = Client::builder();
    if !timeouts.connect.is_zero() {
      builder = builder.connect_timeout(timeouts.connect);
    }
    let client = builder.build().unwrap_or_else(|error| {
      vlog!("Failed to build HTTP client with timeouts: {error}");
      Client::new()
    });
    return HttpClient {
      base_url,
      client,
      timeouts,
    };
  }

  pub async fn get<T>(
    &self,
    endpoint: &str,
    headers: Option<HashMap<String, String>>,
  ) -> Result<T>
  where
    T: serde::de::DeserializeOwned,
  {
    let full_url = self.url_builder(endpoint);
    vlog!("Sending GET request to: {}", full_url);

    let request_builder = self.client.get(&full_url);
    let request_builder = self.apply_timeout(request_builder);
    let request_builder = Self::apply_headers(request_builder, headers);

    let response = request_builder
      .send()
      .await
      .map_err(NetworkError::Transport)
      .context(format!("Failed to send request to '{full_url}'"))?;
    let response = Self::ensure_success(response).await?;

    let parsed_response = response
      .json::<T>()
      .await
      .context("Failed to decode response")?;

    return Ok(parsed_response);
  }

  pub async fn post<T, B>(
    &self,
    body: &B,
    endpoint: &str,
    headers: Option<HashMap<String, String>>,
  ) -> Result<T>
  where
    T: serde::de::DeserializeOwned,
    B: Serialize,
  {
    let full_url = self.url_builder(endpoint);
    vlog!("Sending POST request to: {}", full_url);

    let request_builder = self.client.post(&full_url).json(body);
    let request_builder = self.apply_timeout(request_builder);
    let request_builder = Self::apply_headers(request_builder, headers);

    let response = request_builder
      .send()
      .await
      .map_err(NetworkError::Transport)
      .context(format!("Failed to send request to '{full_url}'"))?;
    let response = Self::ensure_success(response).await?;

    let parsed_response = response
      .json::<T>()
      .await
      .context("Failed to decode response")?;

    return Ok(parsed_response);
  }

  pub async fn post_stream<B>(
    &self,
    body: &B,
    endpoint: &str,
    headers: Option<HashMap<String, String>>,
  ) -> Result<SSEStream>
  where
    B: Serialize,
  {
    let full_url = self.url_builder(endpoint);
    vlog!("Sending POST stream request to: {}", full_url);

    let request_builder = self.client.post(&full_url).json(body);
    let request_builder = Self::apply_headers(request_builder, headers);
    let response = self.send_for_stream(request_builder, &full_url).await?;
    let response = Self::ensure_success(response).await?;

    return Ok(SSEStream::from_response(response));
  }

  async fn send_for_stream(
    &self,
    request_builder: RequestBuilder,
    full_url: &str,
  ) -> Result<Response> {
    if self.timeouts.request.is_zero() {
      return request_builder
        .send()
        .await
        .map_err(NetworkError::Transport)
        .context(format!("Failed to send request to '{full_url}'"));
    }

    let seconds = self.timeouts.request.as_secs();
    match time::timeout(self.timeouts.request, request_builder.send()).await {
      Ok(response) => {
        return response
          .map_err(NetworkError::Transport)
          .context(format!("Failed to send request to '{full_url}'"));
      }
      Err(_) => {
        return Err(NetworkError::Timeout { seconds })
          .context(format!("Failed to send request to '{full_url}'"));
      }
    }
  }

  fn apply_timeout(&self, request_builder: RequestBuilder) -> RequestBuilder {
    if self.timeouts.request.is_zero() {
      return request_builder;
    }
    return request_builder.timeout(self.timeouts.request);
  }

  fn url_builder(&self, endpoint: &str) -> String {
    let base = self.base_url.trim_end_matches('/');
    let ep = endpoint.strip_prefix('/').unwrap_or(endpoint);
    return format!("{base}/{ep}");
  }

  fn apply_headers(
    request_builder: RequestBuilder,
    headers: Option<HashMap<String, String>>,
  ) -> RequestBuilder {
    let request_builder = request_builder
      .header("User-Agent", concat!("Fenrir/", env!("CARGO_PKG_VERSION")));
    if let Some(hdrs) = headers {
      vlog!(
        "Applying headers to request: {:?}",
        Self::redact_headers(&hdrs)
      );
      return hdrs
        .into_iter()
        .fold(request_builder, |builder, (key, value)| {
          builder.header(key, value)
        });
    }
    return request_builder;
  }

  fn redact_headers(
    headers: &HashMap<String, String>,
  ) -> Vec<(String, String)> {
    const SENSITIVE_HEADERS: &[&str] =
      &["Authorization", "X-Api-Key", "X-Token"];
    return headers
      .iter()
      .map(|(k, v)| {
        let redacted =
          if SENSITIVE_HEADERS.iter().any(|s| k.eq_ignore_ascii_case(s)) {
            "[REDACTED]".to_string()
          } else {
            v.clone()
          };
        return (k.clone(), redacted);
      })
      .collect();
  }

  async fn ensure_success(response: Response) -> Result<Response> {
    if response.status().is_success() {
      return Ok(response);
    }

    let status = response.status();
    let retry_after = parse_retry_after(response.headers());
    let raw_body = response
      .text()
      .await
      .unwrap_or_else(|_| String::from("Failed to read response body"));
    let body = service_error_message(&raw_body);
    vlog!("Service returned error {}: {}", status, body);
    return Err(
      NetworkError::Status {
        status,
        body,
        retry_after,
      }
      .into(),
    );
  }
}

fn service_error_message(body: &str) -> String {
  let body = body.trim();
  if body.is_empty() {
    return String::from("the provider returned an empty response body");
  }
  if let Ok(value) = serde_json::from_str::<serde_json::Value>(body)
    && let Some(message) = json_error_message(&value)
  {
    return truncate_message(&message);
  }
  return truncate_message(body);
}

fn json_error_message(value: &serde_json::Value) -> Option<String> {
  if let Some(error) = value.get("error") {
    if let Some(message) = error.as_str() {
      return Some(message.to_string());
    }
    if let Some(message) = error.get("message").and_then(|m| m.as_str()) {
      return Some(message.to_string());
    }
  }
  return value
    .get("message")
    .and_then(|m| m.as_str())
    .map(str::to_string);
}

fn truncate_message(message: &str) -> String {
  const MAX_CHARS: usize = 1000;
  if message.chars().count() <= MAX_CHARS {
    return message.to_string();
  }
  let mut truncated: String = message.chars().take(MAX_CHARS).collect();
  truncated.push_str("...");
  return truncated;
}

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
  let value = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
  let value = value.trim();
  if let Ok(seconds) = value.parse::<u64>() {
    return Some(Duration::from_secs(seconds));
  }
  let date = DateTime::parse_from_rfc2822(value)
    .or_else(|_| DateTime::parse_from_str(value, "%a, %d %b %Y %H:%M:%S GMT"))
    .ok()?;
  let target = date.with_timezone(&Utc);
  return Some((target - Utc::now()).to_std().unwrap_or(Duration::ZERO));
}
