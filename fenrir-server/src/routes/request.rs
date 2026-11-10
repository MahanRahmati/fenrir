use axum::http::StatusCode;
use uuid::Uuid;

use fenrir_core::client::Client;
use fenrir_core::engine::protocol::{ServerEvent, SessionView};

use crate::adapter::next_event;
use crate::api_response::ApiError;

pub async fn load_session(
  client: &mut Client,
  session_id: Uuid,
) -> Result<SessionView, ApiError> {
  let request_id = client.load_session(session_id).await?;
  return await_event(client, request_id, |event| match event {
    ServerEvent::SessionLoaded {
      request_id: id,
      session,
    } if id == request_id => Some(session),
    _ => None,
  })
  .await;
}

pub async fn await_event<T>(
  client: &mut Client,
  request_id: u64,
  mut map: impl FnMut(ServerEvent) -> Option<T>,
) -> Result<T, ApiError> {
  loop {
    let event = next_event(client).await?;
    match &event {
      ServerEvent::Exited => {
        return Err(ApiError::new(
          StatusCode::INTERNAL_SERVER_ERROR,
          "Broker exited",
        ));
      }
      ServerEvent::Error {
        request_id: Some(id),
        message,
        ..
      } if *id == request_id => {
        return Err(ApiError::from_message(message.clone()));
      }
      _ => {}
    }
    if let Some(value) = map(event) {
      return Ok(value);
    }
  }
}
