use axum::response::IntoResponse;
use axum::response::sse::{KeepAlive, Sse};
use futures::stream;
use tokio::sync::mpsc::{self, Receiver};
use uuid::Uuid;

use fenrir_core::engine::protocol::ServerEvent;

use crate::connections::ConnectionsHandle;
use crate::sse::{self, SseEvent};

pub fn connection_stream(
  mut events: Receiver<ServerEvent>,
  connections: ConnectionsHandle,
  client_id: Uuid,
) -> impl IntoResponse {
  let (sender, mut receiver) = mpsc::channel::<SseEvent>(64);
  tokio::spawn(async move {
    loop {
      let event = tokio::select! {
        _ = sender.closed() => break,
        event = events.recv() => match event {
          Some(event) => event,
          None => break,
        },
      };
      let exited = matches!(event, ServerEvent::Exited);
      if sender.send(sse::event_of(&event)).await.is_err() {
        break;
      }
      if exited {
        break;
      }
    }
    let _ = connections.disconnect(client_id).await;
  });
  let stream = stream::poll_fn(move |cx| receiver.poll_recv(cx));
  return Sse::new(stream).keep_alive(KeepAlive::default());
}
