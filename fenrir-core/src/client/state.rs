use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

use crate::engine::protocol::{ServerEvent, TurnOutcome};
use crate::session_manager::QueuedMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
  CreateSession,
  LoadSession,
  ListSessions,
  ListPersonas,
  ListModels,
  SetPersona,
  SetModel,
  SendPrompt,
  RunCommand,
  Compact,
}

#[derive(Debug, Clone)]
pub struct PendingRequest {
  pub kind: RequestKind,
  pub session_id: Option<Uuid>,
  pub starts_turn: bool,
}

#[derive(Debug, Clone)]
pub struct PendingApproval {
  pub session_id: Uuid,
  pub turn_id: Uuid,
  pub tool_call_id: String,
  pub message: String,
}

#[derive(Default)]
pub struct SessionState {
  pub active_turn: Option<Uuid>,
  pub awaiting_usage: Option<Uuid>,
  pub queued_messages: Vec<QueuedMessage>,
  approvals: VecDeque<PendingApproval>,
}

impl SessionState {
  pub fn take_next_approval(&mut self) -> Option<PendingApproval> {
    return self.approvals.pop_front();
  }

  fn clear_approvals(&mut self, turn_id: Uuid) {
    self
      .approvals
      .retain(|approval| approval.turn_id != turn_id);
  }
}

pub struct ClientState {
  pub client_id: Uuid,
  pub visible_session: Option<Uuid>,
  pub creating_session: bool,
  sessions: HashMap<Uuid, SessionState>,
  pending: HashMap<u64, PendingRequest>,
}

impl ClientState {
  pub fn new(client_id: Uuid) -> Self {
    return Self {
      client_id,
      visible_session: None,
      creating_session: false,
      sessions: HashMap::new(),
      pending: HashMap::new(),
    };
  }

  pub fn pending(&self) -> &HashMap<u64, PendingRequest> {
    return &self.pending;
  }

  pub fn is_pending(&self) -> bool {
    return !self.pending.is_empty();
  }

  pub fn pending_kind(&self, request_id: u64) -> Option<RequestKind> {
    return self.pending.get(&request_id).map(|pending| pending.kind);
  }

  pub fn is_busy(&self) -> bool {
    return self.creating_session
      || self.is_pending()
      || self.active_turn().is_some();
  }

  pub fn is_active(&self, session_id: Uuid) -> bool {
    return self.visible_session == Some(session_id);
  }

  pub fn visible_session(&self) -> Option<Uuid> {
    return self.visible_session;
  }

  pub fn session(&self, session_id: Uuid) -> Option<&SessionState> {
    return self.sessions.get(&session_id);
  }

  pub fn ensure_session(&mut self, session_id: Uuid) {
    self.sessions.entry(session_id).or_default();
  }

  pub fn active_turn(&self) -> Option<Uuid> {
    return self
      .visible_session
      .and_then(|session_id| self.sessions.get(&session_id))
      .and_then(|session| session.active_turn);
  }

  pub fn awaiting_usage(&self) -> Option<Uuid> {
    return self
      .visible_session
      .and_then(|session_id| self.sessions.get(&session_id))
      .and_then(|session| session.awaiting_usage);
  }

  pub fn queued_messages(&self) -> &[QueuedMessage] {
    return self
      .visible_session
      .and_then(|session_id| self.sessions.get(&session_id))
      .map(|session| session.queued_messages.as_slice())
      .unwrap_or_default();
  }

  pub fn take_next_approval(&mut self) -> Option<PendingApproval> {
    let session_id = self.visible_session?;
    return self.take_next_approval_for(session_id);
  }

  pub fn take_next_approval_for(
    &mut self,
    session_id: Uuid,
  ) -> Option<PendingApproval> {
    return self
      .sessions
      .get_mut(&session_id)
      .and_then(SessionState::take_next_approval);
  }

  pub fn forget_session(&mut self, session_id: Uuid) {
    self.sessions.remove(&session_id);
    self
      .pending
      .retain(|_, pending| pending.session_id != Some(session_id));
    if self.visible_session == Some(session_id) {
      self.visible_session = None;
    }
  }

  pub(crate) fn insert_pending(
    &mut self,
    request_id: u64,
    pending: PendingRequest,
  ) {
    self.pending.insert(request_id, pending);
  }

  pub(crate) fn remove_pending(&mut self, request_id: u64) -> bool {
    return self.pending.remove(&request_id).is_some();
  }

  fn session_mut(&mut self, session_id: Uuid) -> &mut SessionState {
    return self.sessions.entry(session_id).or_default();
  }

  fn finish_turn_requests(&mut self, session_id: Uuid) {
    self.pending.retain(|_, pending| {
      !pending.starts_turn
        || (pending.session_id.is_some()
          && pending.session_id != Some(session_id))
    });
  }

  pub fn apply(&mut self, event: ServerEvent) -> Option<ServerEvent> {
    let visible_before = self.visible_session;
    self.update(&event);
    if self.is_visible_event(&event, visible_before) {
      return Some(event);
    }
    return None;
  }

  pub fn apply_all(&mut self, event: ServerEvent) -> ServerEvent {
    self.update(&event);
    return event;
  }

  fn update(&mut self, event: &ServerEvent) {
    match event {
      ServerEvent::Connected { client_id } => {
        self.client_id = *client_id;
      }
      ServerEvent::SessionCreated {
        request_id,
        session,
      }
      | ServerEvent::SessionLoaded {
        request_id,
        session,
      } => {
        let keep_pending = self
          .pending
          .get(request_id)
          .is_some_and(|pending| pending.starts_turn);
        if !keep_pending {
          self.remove_pending(*request_id);
        }
        let session_id = session.summary.id;
        self.visible_session = Some(session_id);
        self.creating_session = false;
        let state = self.sessions.entry(session_id).or_default();
        state.awaiting_usage = None;
        state.queued_messages.clear();
        state.approvals.clear();
      }
      ServerEvent::SessionsListed { request_id, .. }
      | ServerEvent::PersonasListed { request_id, .. }
      | ServerEvent::ModelsListed { request_id, .. }
      | ServerEvent::PersonaUpdated { request_id, .. }
      | ServerEvent::ModelUpdated { request_id, .. }
      | ServerEvent::CommandCatalog { request_id, .. } => {
        self.remove_pending(*request_id);
      }
      ServerEvent::UserMessage { .. }
      | ServerEvent::Token { .. }
      | ServerEvent::ToolCall { .. }
      | ServerEvent::ToolResult { .. }
      | ServerEvent::Subagents { .. }
      | ServerEvent::Retrying { .. }
      | ServerEvent::Compacted { .. }
      | ServerEvent::Prediction { .. }
      | ServerEvent::TitleUpdated { .. }
      | ServerEvent::Exited => {}
      ServerEvent::TurnStarted { correlation, .. } => {
        if self.visible_session.is_none() {
          self.visible_session = Some(correlation.session_id);
        }
        self.finish_turn_requests(correlation.session_id);
        let session = self.session_mut(correlation.session_id);
        session.active_turn = Some(correlation.turn_id);
        self.creating_session = false;
      }
      ServerEvent::ToolApprovalRequest {
        correlation,
        tool_call_id,
        message,
        ..
      } => {
        let session = self.session_mut(correlation.session_id);
        session.approvals.push_back(PendingApproval {
          session_id: correlation.session_id,
          turn_id: correlation.turn_id,
          tool_call_id: tool_call_id.clone(),
          message: message.clone(),
        });
      }
      ServerEvent::ToolApprovalResult {
        correlation,
        tool_call_id,
        ..
      } => {
        let session = self.session_mut(correlation.session_id);
        session.approvals.retain(|approval| {
          approval.turn_id != correlation.turn_id
            || approval.tool_call_id != *tool_call_id
        });
      }
      ServerEvent::Usage { correlation, .. } => {
        let session = self.session_mut(correlation.session_id);
        if session.awaiting_usage == Some(correlation.turn_id) {
          session.awaiting_usage = None;
        }
      }
      ServerEvent::QueueUpdated {
        session_id,
        messages,
      } => {
        let session = self.session_mut(*session_id);
        session.queued_messages.clone_from(messages);
      }
      ServerEvent::TurnFinished {
        correlation,
        outcome,
      } => {
        self.finish_turn_requests(correlation.session_id);
        let session = self.session_mut(correlation.session_id);
        if session.active_turn == Some(correlation.turn_id) {
          session.active_turn = None;
        }
        session.clear_approvals(correlation.turn_id);
        session.awaiting_usage = match outcome {
          TurnOutcome::Completed { .. } => Some(correlation.turn_id),
          TurnOutcome::Cancelled | TurnOutcome::Failed { .. } => None,
        };
      }
      ServerEvent::Error { request_id, .. } => {
        if let Some(request_id) = request_id {
          self.remove_pending(*request_id);
        }
        if self.creating_session && self.visible_session.is_none() {
          self.creating_session = false;
        }
      }
      ServerEvent::SessionStopped { session_id } => {
        if self.visible_session == Some(*session_id) {
          self.creating_session = false;
        }
        self.forget_session(*session_id);
      }
    }
  }

  fn is_visible_event(
    &self,
    event: &ServerEvent,
    visible_before: Option<Uuid>,
  ) -> bool {
    return match event {
      ServerEvent::Connected { .. }
      | ServerEvent::SessionsListed { .. }
      | ServerEvent::PersonasListed { .. }
      | ServerEvent::ModelsListed { .. }
      | ServerEvent::CommandCatalog { .. }
      | ServerEvent::Exited => true,
      ServerEvent::SessionCreated { session, .. }
      | ServerEvent::SessionLoaded { session, .. } => {
        self.visible_session == Some(session.summary.id)
      }
      ServerEvent::UserMessage { correlation, .. }
      | ServerEvent::TurnStarted { correlation, .. }
      | ServerEvent::Token { correlation, .. }
      | ServerEvent::ToolCall { correlation, .. }
      | ServerEvent::ToolApprovalRequest { correlation, .. }
      | ServerEvent::ToolApprovalResult { correlation, .. }
      | ServerEvent::ToolResult { correlation, .. }
      | ServerEvent::Usage { correlation, .. }
      | ServerEvent::Subagents { correlation, .. }
      | ServerEvent::Retrying { correlation, .. }
      | ServerEvent::TurnFinished { correlation, .. } => {
        self.visible_session == Some(correlation.session_id)
      }
      ServerEvent::Compacted { session_id, .. }
      | ServerEvent::Prediction { session_id, .. }
      | ServerEvent::TitleUpdated { session_id, .. }
      | ServerEvent::QueueUpdated { session_id, .. } => {
        self.visible_session == Some(*session_id)
      }
      ServerEvent::SessionStopped { session_id } => {
        visible_before == Some(*session_id)
      }
      ServerEvent::PersonaUpdated { session_id, .. }
      | ServerEvent::ModelUpdated { session_id, .. }
      | ServerEvent::Error { session_id, .. } => session_id
        .is_none_or(|session_id| self.visible_session == Some(session_id)),
    };
  }
}
