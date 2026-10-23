pub mod sse;

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use reqwest::{Client, RequestBuilder, Response};
use serde::Serialize;

use crate::network::sse::SSEStream;
use crate::vlog;

#[derive(Debug, Clone)]
pub struct HttpClient {
  base_url: String,
  client: Client,
}

impl HttpClient {
  pub fn new(base_url: String) -> Self {
    let client = Client::new();
    return HttpClient { base_url, client };
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
    let request_builder = Self::apply_headers(request_builder, headers);

    let response = request_builder
      .send()
      .await
      .context(format!("Failed to send request to '{full_url}'"))?;

    if !response.status().is_success() {
      let status = response.status().as_str().to_string();
      let body_text = response
        .text()
        .await
        .unwrap_or_else(|_| String::from("Failed to read response body"));
      vlog!("Service returned error {}: {}", status, body_text);
      bail!("Service returned {status}: {body_text}");
    }

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

    let response = request_builder
      .send()
      .await
      .context(format!("Failed to send request to '{full_url}'"))?;

    return Self::response_to_stream(response).await;
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
      vlog!("Applying headers to request: {:?}", hdrs);
      return hdrs
        .into_iter()
        .fold(request_builder, |builder, (key, value)| {
          builder.header(key, value)
        });
    }
    return request_builder;
  }

  async fn response_to_stream(response: Response) -> Result<SSEStream> {
    vlog!("Received streaming response. Status: {}", response.status());

    if !response.status().is_success() {
      let status = response.status().as_str().to_string();
      let body = response
        .text()
        .await
        .unwrap_or_else(|_| String::from("Failed to read response body"));
      vlog!("Service returned error {}: {}", status, body);
      bail!("Service returned {status}: {body}");
    }

    return Ok(SSEStream::from_response(response));
  }
}
