use std::pin::Pin;

use anyhow::{Context, Result, bail};
use futures::{Stream, StreamExt};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::client::api::{
  ApiResponse, ApprovalRequest, CancelRequest, CommandsData, CompactRequest,
  ConnectionData, CreateConnectionRequest, CreateSessionRequest, DefaultsData,
  ModelRequest, ModelsData, PersonaRequest, PersonasData, PromptRequest,
  ReviewRequest, SessionsData, SubscribeRequest,
};
use crate::client::state::{
  ClientState, PendingApproval, PendingRequest, RequestKind,
};
use crate::client::urls;
use crate::engine::protocol::{
  CommandInfo, PersonaInfo, PromptMode, ServerEvent, SessionSummary,
};
use crate::provider_manager::Model;

type EventStream = Pin<Box<dyn Stream<Item = reqwest::Result<Vec<u8>>> + Send>>;

pub struct ApiClient {
  http: reqwest::Client,
  base_url: String,
  client_id: Uuid,
  state: ClientState,
  next_request_id: u64,
  events: EventStream,
  buffer: Vec<u8>,
  pending_unsubscribe: Option<Uuid>,
}

impl ApiClient {
  pub async fn connect(base_url: &str, cwd: &str) -> Result<Self> {
    let http = reqwest::Client::new();
    let response = http
      .post(format!("{base_url}{}", urls::CONNECTIONS))
      .json(&CreateConnectionRequest {
        cwd: Some(cwd.to_string()),
      })
      .send()
      .await
      .context(format!("Cannot reach the server at {base_url}"))?;
    let created: ApiResponse<ConnectionData> = parse(response).await?;
    let client_id = created
      .data
      .context("The server did not return a connection")?
      .client_id;

    let stream = http
      .get(format!("{base_url}{}", urls::connection_events(client_id)))
      .send()
      .await
      .context("Cannot open the event stream")?
      .error_for_status()
      .context("Cannot open the event stream")?;

    return Ok(Self {
      http,
      base_url: base_url.to_string(),
      client_id,
      state: ClientState::new(client_id),
      next_request_id: 1,
      events: Box::pin(
        stream
          .bytes_stream()
          .map(|chunk| chunk.map(|bytes| bytes.to_vec())),
      ),
      buffer: Vec::new(),
      pending_unsubscribe: None,
    });
  }

  pub fn state(&self) -> &ClientState {
    return &self.state;
  }

  pub fn take_next_approval(&mut self) -> Option<PendingApproval> {
    return self.state.take_next_approval();
  }

  pub async fn defaults(&self) -> Result<DefaultsData> {
    return self.get(urls::DEFAULTS).await;
  }

  pub async fn sessions(&self) -> Result<Vec<SessionSummary>> {
    let data: SessionsData = self.get(urls::SESSIONS).await?;
    return Ok(data.sessions);
  }

  pub async fn personas(&self) -> Result<Vec<PersonaInfo>> {
    let data: PersonasData = self.get(urls::PERSONAS).await?;
    return Ok(data.personas);
  }

  pub async fn models(&self) -> Result<Vec<Model>> {
    let data: ModelsData = self.get(urls::MODELS).await?;
    return Ok(data.models);
  }

  pub async fn commands(
    &self,
    session_id: Option<Uuid>,
  ) -> Result<Vec<CommandInfo>> {
    let data: CommandsData = self.get(&urls::commands(session_id)).await?;
    return Ok(data.commands);
  }

  pub async fn create_session(
    &mut self,
    persona: Option<String>,
    model: Option<Model>,
  ) -> Result<u64> {
    let request_id = self.take_request_id();
    let body = CreateSessionRequest {
      request_id,
      persona,
      model,
    };
    let path = urls::connection_sessions(self.client_id);
    self
      .post_tracked(
        &path,
        &body,
        request_id,
        RequestKind::CreateSession,
        None,
        false,
      )
      .await?;
    return Ok(request_id);
  }

  pub async fn load_session(&mut self, session_id: Uuid) -> Result<u64> {
    let request_id = self.take_request_id();
    let path = urls::connection_session(self.client_id, session_id);
    let request = self
      .http
      .put(self.url(&path))
      .json(&SubscribeRequest { request_id });
    self
      .send_tracked(
        request,
        request_id,
        RequestKind::LoadSession,
        Some(session_id),
        false,
      )
      .await?;
    self.state.ensure_session(session_id);
    return Ok(request_id);
  }

  pub async fn send_prompt(&mut self, text: impl Into<String>) -> Result<u64> {
    let request_id = self.take_request_id();
    let session_id = self.state.visible_session();
    if session_id.is_none() {
      self.state.creating_session = true;
    }
    let body = PromptRequest {
      request_id,
      session_id,
      text: text.into(),
      mode: Some(PromptMode::Normal),
    };
    let path = urls::connection_prompts(self.client_id);
    if let Err(error) = self
      .post_tracked(
        &path,
        &body,
        request_id,
        RequestKind::SendPrompt,
        session_id,
        true,
      )
      .await
    {
      self.state.creating_session = false;
      return Err(error);
    }
    return Ok(request_id);
  }

  pub async fn set_persona_for(
    &mut self,
    session_id: Option<Uuid>,
    name: impl Into<String>,
  ) -> Result<u64> {
    let request_id = self.take_request_id();
    let body = PersonaRequest {
      request_id,
      session_id,
      name: name.into(),
    };
    let path = urls::connection_persona(self.client_id);
    self
      .post_tracked(
        &path,
        &body,
        request_id,
        RequestKind::SetPersona,
        session_id,
        false,
      )
      .await?;
    return Ok(request_id);
  }

  pub async fn set_model_for(
    &mut self,
    session_id: Option<Uuid>,
    model: Model,
  ) -> Result<u64> {
    let request_id = self.take_request_id();
    let body = ModelRequest {
      request_id,
      session_id,
      model,
    };
    let path = urls::connection_model(self.client_id);
    self
      .post_tracked(
        &path,
        &body,
        request_id,
        RequestKind::SetModel,
        session_id,
        false,
      )
      .await?;
    return Ok(request_id);
  }

  pub async fn review(&mut self, session_id: Uuid) -> Result<u64> {
    let request_id = self.take_request_id();
    let path = urls::connection_session_review(self.client_id, session_id);
    self
      .post_tracked(
        &path,
        &ReviewRequest { request_id },
        request_id,
        RequestKind::RunCommand,
        Some(session_id),
        true,
      )
      .await?;
    return Ok(request_id);
  }

  pub async fn compact(&mut self) -> Result<u64> {
    let Some(session_id) = self.state.visible_session() else {
      bail!("No session selected");
    };
    let request_id = self.take_request_id();
    let path = urls::connection_session_compact(self.client_id, session_id);
    self
      .post_tracked(
        &path,
        &CompactRequest { request_id },
        request_id,
        RequestKind::Compact,
        Some(session_id),
        true,
      )
      .await?;
    return Ok(request_id);
  }

  pub async fn cancel(&mut self, turn_id: Option<Uuid>) -> Result<()> {
    let Some(session_id) = self.state.visible_session() else {
      return Ok(());
    };
    let path = urls::connection_session_cancel(self.client_id, session_id);
    return self.post(&path, &CancelRequest { turn_id }).await;
  }

  pub async fn approve_tool(
    &mut self,
    session_id: Uuid,
    turn_id: Uuid,
    tool_call_id: String,
    approved: bool,
  ) -> Result<()> {
    let path = urls::connection_session_approval(self.client_id, session_id);
    let body = ApprovalRequest {
      turn_id,
      tool_call_id,
      approved,
    };
    return self.post(&path, &body).await;
  }

  pub async fn predict(&mut self, session_id: Uuid) -> Result<()> {
    let path = urls::connection_session_predict(self.client_id, session_id);
    return self.post(&path, &serde_json::json!({})).await;
  }

  pub async fn disconnect(&mut self) -> Result<()> {
    let path = urls::connection(self.client_id);
    let response = self
      .http
      .delete(self.url(&path))
      .send()
      .await
      .context("Cannot close the connection")?;
    parse::<serde_json::Value>(response).await?;
    return Ok(());
  }

  pub async fn recv(&mut self) -> Option<ServerEvent> {
    self.flush_unsubscribe().await;
    loop {
      let event = self.next_event().await?;
      let request_error = matches!(
        &event,
        ServerEvent::Error {
          request_id: Some(_),
          ..
        }
      );
      let previous_session = self.state.visible_session();
      let update = if request_error {
        Some(self.state.apply_all(event))
      } else {
        self.state.apply(event)
      };
      let Some(update) = update else {
        continue;
      };
      if let ServerEvent::SessionCreated { session, .. }
      | ServerEvent::SessionLoaded { session, .. } = &update
        && let Some(previous_session) = previous_session
        && previous_session != session.summary.id
      {
        self.state.forget_session(previous_session);
        self.pending_unsubscribe = Some(previous_session);
      }
      return Some(update);
    }
  }

  async fn flush_unsubscribe(&mut self) {
    let Some(session_id) = self.pending_unsubscribe else {
      return;
    };
    let path = urls::connection_session(self.client_id, session_id);
    if self.http.delete(self.url(&path)).send().await.is_ok() {
      self.pending_unsubscribe = None;
    }
  }

  fn take_request_id(&mut self) -> u64 {
    let request_id = self.next_request_id;
    self.next_request_id += 1;
    return request_id;
  }

  fn url(&self, path: &str) -> String {
    return format!("{}{path}", self.base_url);
  }

  async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
    let response = self
      .http
      .get(self.url(path))
      .send()
      .await
      .context("Cannot reach the Fenrir server")?;
    let body: ApiResponse<T> = parse(response).await?;
    return body.data.context("The server returned no data");
  }

  async fn post<T: Serialize>(&self, path: &str, body: &T) -> Result<()> {
    let response = self
      .http
      .post(self.url(path))
      .json(body)
      .send()
      .await
      .context("Cannot reach the Fenrir server")?;
    parse::<serde_json::Value>(response).await?;
    return Ok(());
  }

  async fn post_tracked<T: Serialize>(
    &mut self,
    path: &str,
    body: &T,
    request_id: u64,
    kind: RequestKind,
    session_id: Option<Uuid>,
    starts_turn: bool,
  ) -> Result<()> {
    let request = self.http.post(self.url(path)).json(body);
    return self
      .send_tracked(request, request_id, kind, session_id, starts_turn)
      .await;
  }

  async fn send_tracked(
    &mut self,
    request: reqwest::RequestBuilder,
    request_id: u64,
    kind: RequestKind,
    session_id: Option<Uuid>,
    starts_turn: bool,
  ) -> Result<()> {
    self.state.insert_pending(
      request_id,
      PendingRequest {
        kind,
        session_id,
        starts_turn,
      },
    );
    let response = match request.send().await {
      Ok(response) => response,
      Err(error) => {
        self.state.remove_pending(request_id);
        return Err(error).context("Cannot reach the Fenrir server");
      }
    };
    if let Err(error) = parse::<serde_json::Value>(response).await {
      self.state.remove_pending(request_id);
      return Err(error);
    }
    return Ok(());
  }

  async fn next_event(&mut self) -> Option<ServerEvent> {
    loop {
      if let Some(position) =
        self.buffer.windows(2).position(|window| window == b"\n\n")
      {
        let block: Vec<u8> = self.buffer.drain(..position + 2).collect();
        if let Some(event) = parse_block(&block) {
          return Some(event);
        }
        continue;
      }
      match self.events.next().await? {
        Ok(chunk) => self.buffer.extend_from_slice(&chunk),
        Err(_) => return None,
      }
    }
  }
}

async fn parse<T: DeserializeOwned>(
  response: reqwest::Response,
) -> Result<ApiResponse<T>> {
  let status = response.status();
  let body: ApiResponse<T> = response
    .json()
    .await
    .context("Cannot read the server response")?;
  if !status.is_success() {
    return Err(anyhow::anyhow!(
      "{}",
      body.message.unwrap_or_else(|| status.to_string())
    ));
  }
  return Ok(body);
}

fn parse_block(block: &[u8]) -> Option<ServerEvent> {
  let text = std::str::from_utf8(block).ok()?;
  let mut data = String::new();
  for line in text.lines() {
    let Some(rest) = line.strip_prefix("data:") else {
      continue;
    };
    data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
  }
  if data.is_empty() {
    return None;
  }
  return serde_json::from_str(&data).ok();
}
