use uuid::Uuid;

pub const HEALTH: &str = "/api/health";
pub const DEFAULTS: &str = "/api/defaults";
pub const CONNECTIONS: &str = "/api/connections";
pub const SESSIONS: &str = "/api/sessions";
pub const SESSION: &str = "/api/sessions/{session_id}";
pub const MODELS: &str = "/api/models";
pub const PERSONAS: &str = "/api/personas";
pub const COMMANDS: &str = "/api/commands";

pub const CONNECTION: &str = "/api/connections/{client_id}";
pub const CONNECTION_EVENTS: &str = "/api/connections/{client_id}/events";
pub const CONNECTION_SESSIONS: &str = "/api/connections/{client_id}/sessions";
pub const CONNECTION_PROMPTS: &str = "/api/connections/{client_id}/prompts";
pub const CONNECTION_PERSONA: &str = "/api/connections/{client_id}/persona";
pub const CONNECTION_MODEL: &str = "/api/connections/{client_id}/model";

pub const CONNECTION_SESSION: &str =
  "/api/connections/{client_id}/sessions/{session_id}";
pub const CONNECTION_SESSION_REVIEW: &str =
  "/api/connections/{client_id}/sessions/{session_id}/review";
pub const CONNECTION_SESSION_COMPACT: &str =
  "/api/connections/{client_id}/sessions/{session_id}/compact";
pub const CONNECTION_SESSION_CANCEL: &str =
  "/api/connections/{client_id}/sessions/{session_id}/cancel";
pub const CONNECTION_SESSION_APPROVAL: &str =
  "/api/connections/{client_id}/sessions/{session_id}/approval";
pub const CONNECTION_SESSION_PREDICT: &str =
  "/api/connections/{client_id}/sessions/{session_id}/predict";

pub fn connection(client_id: Uuid) -> String {
  return fill_client(CONNECTION, client_id);
}

pub fn connection_events(client_id: Uuid) -> String {
  return fill_client(CONNECTION_EVENTS, client_id);
}

pub fn connection_sessions(client_id: Uuid) -> String {
  return fill_client(CONNECTION_SESSIONS, client_id);
}

pub fn connection_prompts(client_id: Uuid) -> String {
  return fill_client(CONNECTION_PROMPTS, client_id);
}

pub fn connection_persona(client_id: Uuid) -> String {
  return fill_client(CONNECTION_PERSONA, client_id);
}

pub fn connection_model(client_id: Uuid) -> String {
  return fill_client(CONNECTION_MODEL, client_id);
}

pub fn connection_session(client_id: Uuid, session_id: Uuid) -> String {
  return fill_session(CONNECTION_SESSION, client_id, session_id);
}

pub fn connection_session_review(client_id: Uuid, session_id: Uuid) -> String {
  return fill_session(CONNECTION_SESSION_REVIEW, client_id, session_id);
}

pub fn connection_session_compact(client_id: Uuid, session_id: Uuid) -> String {
  return fill_session(CONNECTION_SESSION_COMPACT, client_id, session_id);
}

pub fn connection_session_cancel(client_id: Uuid, session_id: Uuid) -> String {
  return fill_session(CONNECTION_SESSION_CANCEL, client_id, session_id);
}

pub fn connection_session_approval(
  client_id: Uuid,
  session_id: Uuid,
) -> String {
  return fill_session(CONNECTION_SESSION_APPROVAL, client_id, session_id);
}

pub fn connection_session_predict(client_id: Uuid, session_id: Uuid) -> String {
  return fill_session(CONNECTION_SESSION_PREDICT, client_id, session_id);
}

pub fn session(session_id: Uuid) -> String {
  return SESSION.replace("{session_id}", &session_id.to_string());
}

pub fn commands(session_id: Option<Uuid>) -> String {
  return match session_id {
    Some(session_id) => format!("{COMMANDS}?session_id={session_id}"),
    None => COMMANDS.to_string(),
  };
}

fn fill_client(template: &str, client_id: Uuid) -> String {
  return template.replace("{client_id}", &client_id.to_string());
}

fn fill_session(template: &str, client_id: Uuid, session_id: Uuid) -> String {
  return fill_client(template, client_id)
    .replace("{session_id}", &session_id.to_string());
}
