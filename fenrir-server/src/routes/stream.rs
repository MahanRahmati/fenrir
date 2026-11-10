use axum::response::IntoResponse;
use axum::response::sse::{KeepAlive, Sse};
use futures::stream;
use tokio::sync::mpsc::{self, Sender};
use uuid::Uuid;

use fenrir_core::client::Client;
use fenrir_core::engine::protocol::{ServerEvent, TurnOutcome};

use crate::sse::{self, SseEvent};

pub enum StreamEnd {
  Live,
  Session { request_id: u64 },
  Request { request_id: u64 },
  Turn { request_id: u64, client_id: Uuid },
}

pub fn session_stream(
  client: Client,
  session_id: Uuid,
  end: StreamEnd,
) -> impl IntoResponse {
  let (events, mut receiver) = mpsc::channel(64);
  tokio::spawn(forward(client, session_id, end, events));
  let stream = stream::poll_fn(move |cx| receiver.poll_recv(cx));
  return Sse::new(stream).keep_alive(KeepAlive::default());
}

async fn forward(
  mut client: Client,
  session_id: Uuid,
  end: StreamEnd,
  events: Sender<SseEvent>,
) {
  let mut turn_id = None;
  let mut awaiting_usage = false;
  loop {
    let event = tokio::select! {
      _ = events.closed() => break,
      event = client.recv_all() => match event {
        Some(event) => event,
        None => break,
      },
    };
    let close = should_close_stream(
      &event,
      session_id,
      &end,
      &mut turn_id,
      &mut awaiting_usage,
    );
    if events.send(sse::event_of(&event)).await.is_err() {
      break;
    }
    if close {
      break;
    }
  }
  let _ = client.disconnect().await;
}

fn should_close_stream(
  event: &ServerEvent,
  session_id: Uuid,
  end: &StreamEnd,
  turn_id: &mut Option<Uuid>,
  awaiting_usage: &mut bool,
) -> bool {
  if matches!(
    event,
    ServerEvent::SessionStopped { session_id: id } if *id == session_id
  ) || matches!(event, ServerEvent::Exited)
  {
    return true;
  }
  let request_id = match end {
    StreamEnd::Live => None,
    StreamEnd::Session { request_id }
    | StreamEnd::Request { request_id }
    | StreamEnd::Turn { request_id, .. } => Some(*request_id),
  };
  if let Some(request_id) = request_id
    && matches!(
      event,
      ServerEvent::Error {
        request_id: Some(id),
        ..
      } if *id == request_id
    )
  {
    return true;
  }
  return match end {
    StreamEnd::Live | StreamEnd::Session { .. } => false,
    StreamEnd::Request { request_id } => {
      response_request_id(event) == Some(*request_id)
    }
    StreamEnd::Turn { client_id, .. } => match event {
      ServerEvent::TurnStarted {
        correlation,
        client_id: id,
      } if *id == *client_id && correlation.session_id == session_id => {
        *turn_id = Some(correlation.turn_id);
        false
      }
      ServerEvent::TurnFinished {
        correlation,
        outcome,
      } if Some(correlation.turn_id) == *turn_id => {
        if matches!(outcome, TurnOutcome::Completed { .. }) {
          *awaiting_usage = true;
          false
        } else {
          true
        }
      }
      ServerEvent::Usage { correlation, .. }
        if *awaiting_usage && Some(correlation.turn_id) == *turn_id =>
      {
        true
      }
      _ => false,
    },
  };
}

fn response_request_id(event: &ServerEvent) -> Option<u64> {
  return match event {
    ServerEvent::SessionCreated { request_id, .. }
    | ServerEvent::SessionLoaded { request_id, .. }
    | ServerEvent::SessionsListed { request_id, .. }
    | ServerEvent::PersonasListed { request_id, .. }
    | ServerEvent::ModelsListed { request_id, .. }
    | ServerEvent::PersonaUpdated { request_id, .. }
    | ServerEvent::ModelUpdated { request_id, .. }
    | ServerEvent::CommandCatalog { request_id, .. } => Some(*request_id),
    _ => None,
  };
}
