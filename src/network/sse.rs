use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::Stream;

use crate::network::errors::NetworkError;

pub struct SSEStream {
  body: Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>,
  buffer: String,
}

#[derive(Debug, Clone)]
pub struct SSEChunk {
  pub event: String,
  pub data: String,
}

impl SSEStream {
  pub fn from_response(response: reqwest::Response) -> Self {
    return Self {
      body: Box::pin(response.bytes_stream()),
      buffer: String::new(),
    };
  }
}

impl Stream for SSEStream {
  type Item = Result<SSEChunk, NetworkError>;

  fn poll_next(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Self::Item>> {
    loop {
      if let Some(end_pos) = self.buffer.find("\n\n") {
        let message = self.buffer[..end_pos].to_string();
        self.buffer.drain(..end_pos + 2);
        let chunk = parse_sse_message(&message);
        return Poll::Ready(Some(Ok(chunk)));
      }

      match Pin::new(&mut self.body).poll_next(cx) {
        Poll::Ready(Some(chunk_result)) => match chunk_result {
          Ok(bytes) => {
            if bytes.is_empty() {
              continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            self.buffer.push_str(&text);
          }
          Err(e) => {
            return Poll::Ready(Some(Err(NetworkError::RequestFailed(
              e.to_string(),
            ))));
          }
        },
        Poll::Ready(None) => {
          if self.buffer.is_empty() {
            return Poll::Ready(None);
          }
          let message = self.buffer.to_string();
          self.buffer.clear();
          let chunk = parse_sse_message(&message);
          return Poll::Ready(Some(Ok(chunk)));
        }
        Poll::Pending => {
          return Poll::Pending;
        }
      }
    }
  }
}

fn parse_sse_message(message: &str) -> SSEChunk {
  let mut event = String::new();
  let mut data = String::new();

  for line in message.lines() {
    if line.starts_with("event:") {
      event = line.strip_prefix("event:").unwrap_or("").trim().to_string();
    }
    if line.starts_with("data:") {
      let value = line.strip_prefix("data:").unwrap_or("").trim();
      if !data.is_empty() {
        data.push('\n');
      }
      data.push_str(value);
    }
  }

  if event.is_empty() {
    event = "message".to_string();
  }

  return SSEChunk { event, data };
}
