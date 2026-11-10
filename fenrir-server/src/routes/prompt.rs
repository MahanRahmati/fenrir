use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{KeepAlive, Sse};
use futures::stream;
use serde::Deserialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use fenrir_core::client::Client;
use fenrir_core::engine::protocol::ServerEvent;

use crate::adapter::{AdapterHandle, next_event};
use crate::api_request::ApiRequest;
use crate::api_response::ApiError;
use crate::sse::{self, SseEvent};

#[derive(Debug, Deserialize)]
pub struct PromptRequest {
  pub text: String,
}

impl ApiRequest for PromptRequest {
  fn validate(&self) -> Result<(), ApiError> {
    if self.text.trim().is_empty() {
      return Err(ApiError::new(
        StatusCode::BAD_REQUEST,
        "Prompt text is required",
      ));
    }
    return Ok(());
  }
}

pub async fn prompt(
  State(adapter): State<AdapterHandle>,
  Path(session_id): Path<Uuid>,
  Json(request): Json<PromptRequest>,
) -> Result<impl IntoResponse, ApiError> {
  request.validate()?;

  let mut client = adapter.connect().await?;
  if let Err(error) = load_session(&mut client, session_id).await {
    let _ = client.disconnect().await;
    return Err(error);
  }

  let request_id = match client.send_prompt(request.text).await {
    Ok(request_id) => request_id,
    Err(error) => {
      let _ = client.disconnect().await;
      return Err(error.into());
    }
  };

  let (events, mut receiver) = mpsc::channel(64);
  tokio::spawn(stream_session(client, session_id, request_id, events));
  let stream = stream::poll_fn(move |cx| receiver.poll_recv(cx));
  return Ok(Sse::new(stream).keep_alive(KeepAlive::default()));
}

async fn load_session(
  client: &mut Client,
  session_id: Uuid,
) -> Result<(), ApiError> {
  let request_id = client.load_session(session_id).await?;
  loop {
    match next_event(client).await? {
      ServerEvent::SessionLoaded { request_id: id, .. } if id == request_id => {
        return Ok(());
      }
      ServerEvent::Error {
        request_id: Some(id),
        message,
        ..
      } if id == request_id => {
        let status = if message == "Session not found" {
          StatusCode::NOT_FOUND
        } else {
          StatusCode::INTERNAL_SERVER_ERROR
        };
        return Err(ApiError::new(status, message));
      }
      _ => {}
    }
  }
}

async fn stream_session(
  mut client: Client,
  session_id: Uuid,
  prompt_request_id: u64,
  events: mpsc::Sender<SseEvent>,
) {
  loop {
    let event = tokio::select! {
      _ = events.closed() => break,
      event = client.recv_all() => match event {
        Some(event) => event,
        None => break,
      },
    };
    let terminal = match &event {
      ServerEvent::Error {
        request_id: Some(id),
        ..
      } if *id == prompt_request_id => true,
      ServerEvent::SessionStopped { session_id: id } if *id == session_id => {
        true
      }
      ServerEvent::Exited => true,
      _ => false,
    };
    if events.send(sse::event_of(&event)).await.is_err() {
      break;
    }
    if terminal {
      break;
    }
  }
  let _ = client.disconnect().await;
}
