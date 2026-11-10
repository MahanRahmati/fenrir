use std::convert::Infallible;

use axum::response::sse::Event;

use fenrir_core::engine::protocol::ServerEvent;

pub type SseEvent = Result<Event, Infallible>;

pub fn event_of(event: &ServerEvent) -> SseEvent {
  return Ok(
    Event::default()
      .event(event.name())
      .data(event.data().to_string()),
  );
}
