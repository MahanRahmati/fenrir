use std::convert::Infallible;

use axum::response::sse::Event;

use fenrir_core::engine::protocol::ServerEvent;

pub type SseEvent = Result<Event, Infallible>;

pub fn event_of(event: &ServerEvent) -> SseEvent {
  let data =
    serde_json::to_string(event).unwrap_or_else(|_| String::from("{}"));
  return Ok(Event::default().event(event.name()).data(data));
}
