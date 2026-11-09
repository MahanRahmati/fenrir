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

pub struct ClientState {
  pub client_id: Uuid,
  pub active_session: Option<Uuid>,
  pub active_turn: Option<Uuid>,
  pub creating_session: bool,
  pub awaiting_usage: Option<Uuid>,
  pub queued_messages: Vec<QueuedMessage>,
  pending: HashMap<u64, PendingRequest>,
  approvals: VecDeque<PendingApproval>,
}

impl ClientState {
  pub fn new(client_id: Uuid) -> Self {
    return Self {
      client_id,
      active_session: None,
      active_turn: None,
      creating_session: false,
      awaiting_usage: None,
      queued_messages: Vec::new(),
      pending: HashMap::new(),
      approvals: VecDeque::new(),
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
      || self.active_turn.is_some();
  }

  pub fn is_active(&self, session_id: Uuid) -> bool {
    return self.active_session == Some(session_id);
  }

  pub fn approvals(&self) -> &VecDeque<PendingApproval> {
    return &self.approvals;
  }

  pub fn take_next_approval(&mut self) -> Option<PendingApproval> {
    return self.approvals.pop_front();
  }

  pub fn clear_approvals(&mut self, session_id: Uuid, turn_id: Uuid) {
    self.approvals.retain(|approval| {
      approval.session_id != session_id || approval.turn_id != turn_id
    });
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

  fn finish_turn_requests(&mut self, session_id: Uuid) {
    self.pending.retain(|_, pending| {
      !pending.starts_turn
        || (pending.session_id.is_some()
          && pending.session_id != Some(session_id))
    });
  }

  pub fn apply(&mut self, event: ServerEvent) -> Option<ServerEvent> {
    match event {
      ServerEvent::Connected { client_id } => {
        self.client_id = client_id;
        return Some(ServerEvent::Connected { client_id });
      }
      ServerEvent::SessionCreated {
        request_id,
        session,
      } => {
        let keep_pending = self
          .pending
          .get(&request_id)
          .is_some_and(|pending| pending.starts_turn);
        if !keep_pending {
          self.remove_pending(request_id);
        }
        self.active_session = Some(session.summary.id);
        self.active_turn = None;
        self.creating_session = false;
        self.awaiting_usage = None;
        self.queued_messages.clear();
        self.approvals.clear();
        return Some(ServerEvent::SessionCreated {
          request_id,
          session,
        });
      }
      ServerEvent::SessionLoaded {
        request_id,
        session,
      } => {
        self.remove_pending(request_id);
        self.active_session = Some(session.summary.id);
        self.active_turn = None;
        self.creating_session = false;
        self.awaiting_usage = None;
        self.queued_messages.clear();
        self.approvals.clear();
        return Some(ServerEvent::SessionLoaded {
          request_id,
          session,
        });
      }
      ServerEvent::SessionsListed {
        request_id,
        sessions,
      } => {
        self.remove_pending(request_id);
        return Some(ServerEvent::SessionsListed {
          request_id,
          sessions,
        });
      }
      ServerEvent::PersonasListed {
        request_id,
        personas,
        current,
      } => {
        self.remove_pending(request_id);
        return Some(ServerEvent::PersonasListed {
          request_id,
          personas,
          current,
        });
      }
      ServerEvent::ModelsListed {
        request_id,
        models,
        current,
      } => {
        self.remove_pending(request_id);
        return Some(ServerEvent::ModelsListed {
          request_id,
          models,
          current,
        });
      }
      ServerEvent::PersonaUpdated {
        request_id,
        session_id,
        persona,
      } => {
        self.remove_pending(request_id);
        return Some(ServerEvent::PersonaUpdated {
          request_id,
          session_id,
          persona,
        });
      }
      ServerEvent::ModelUpdated {
        request_id,
        session_id,
        model,
      } => {
        self.remove_pending(request_id);
        return Some(ServerEvent::ModelUpdated {
          request_id,
          session_id,
          model,
        });
      }
      ServerEvent::CommandCatalog {
        request_id,
        commands,
      } => {
        self.remove_pending(request_id);
        return Some(ServerEvent::CommandCatalog {
          request_id,
          commands,
        });
      }
      ServerEvent::UserMessage {
        correlation,
        client_id,
        content,
      } => {
        if !self.is_active(correlation.session_id) {
          return None;
        }
        return Some(ServerEvent::UserMessage {
          correlation,
          client_id,
          content,
        });
      }
      ServerEvent::TurnStarted {
        correlation,
        client_id,
      } => {
        if self.active_session.is_none() {
          self.active_session = Some(correlation.session_id);
        }
        if !self.is_active(correlation.session_id) {
          return None;
        }
        self.finish_turn_requests(correlation.session_id);
        self.active_turn = Some(correlation.turn_id);
        self.creating_session = false;
        return Some(ServerEvent::TurnStarted {
          correlation,
          client_id,
        });
      }
      ServerEvent::Token {
        correlation,
        text,
        reasoning,
      } => {
        if !self.is_active(correlation.session_id) {
          return None;
        }
        return Some(ServerEvent::Token {
          correlation,
          text,
          reasoning,
        });
      }
      ServerEvent::ToolCall {
        correlation,
        id,
        name,
        arguments,
        preview,
      } => {
        if !self.is_active(correlation.session_id) {
          return None;
        }
        return Some(ServerEvent::ToolCall {
          correlation,
          id,
          name,
          arguments,
          preview,
        });
      }
      ServerEvent::ToolApprovalRequest {
        correlation,
        tool_call_id,
        tool_name,
        message,
      } => {
        if !self.is_active(correlation.session_id) {
          return None;
        }
        self.approvals.push_back(PendingApproval {
          session_id: correlation.session_id,
          turn_id: correlation.turn_id,
          tool_call_id: tool_call_id.clone(),
          message: message.clone(),
        });
        return Some(ServerEvent::ToolApprovalRequest {
          correlation,
          tool_call_id,
          tool_name,
          message,
        });
      }
      ServerEvent::ToolApprovalResult {
        correlation,
        tool_call_id,
        status,
      } => {
        self.approvals.retain(|approval| {
          approval.session_id != correlation.session_id
            || approval.turn_id != correlation.turn_id
            || approval.tool_call_id != tool_call_id
        });
        if !self.is_active(correlation.session_id) {
          return None;
        }
        return Some(ServerEvent::ToolApprovalResult {
          correlation,
          tool_call_id,
          status,
        });
      }
      ServerEvent::ToolResult {
        correlation,
        id,
        result,
        error,
      } => {
        if !self.is_active(correlation.session_id) {
          return None;
        }
        return Some(ServerEvent::ToolResult {
          correlation,
          id,
          result,
          error,
        });
      }
      ServerEvent::Usage {
        correlation,
        usage,
        context_window,
      } => {
        if self.awaiting_usage == Some(correlation.turn_id) {
          self.awaiting_usage = None;
        }
        if !self.is_active(correlation.session_id) {
          return None;
        }
        return Some(ServerEvent::Usage {
          correlation,
          usage,
          context_window,
        });
      }
      ServerEvent::Subagents { correlation, delta } => {
        if !self.is_active(correlation.session_id) {
          return None;
        }
        return Some(ServerEvent::Subagents { correlation, delta });
      }
      ServerEvent::Compacted {
        session_id,
        summary,
      } => {
        if !self.is_active(session_id) {
          return None;
        }
        return Some(ServerEvent::Compacted {
          session_id,
          summary,
        });
      }
      ServerEvent::Prediction { session_id, text } => {
        if !self.is_active(session_id) {
          return None;
        }
        return Some(ServerEvent::Prediction { session_id, text });
      }
      ServerEvent::TitleUpdated { session_id, title } => {
        if !self.is_active(session_id) {
          return None;
        }
        return Some(ServerEvent::TitleUpdated { session_id, title });
      }
      ServerEvent::QueueUpdated {
        session_id,
        messages,
      } => {
        if !self.is_active(session_id) {
          return None;
        }
        self.queued_messages = messages.clone();
        return Some(ServerEvent::QueueUpdated {
          session_id,
          messages,
        });
      }
      ServerEvent::TurnFinished {
        correlation,
        outcome,
      } => {
        let is_active = self.is_active(correlation.session_id);
        self.finish_turn_requests(correlation.session_id);
        if self.active_turn == Some(correlation.turn_id) {
          self.active_turn = None;
        }
        self.clear_approvals(correlation.session_id, correlation.turn_id);
        if is_active {
          self.awaiting_usage = match outcome {
            TurnOutcome::Completed { .. } => Some(correlation.turn_id),
            TurnOutcome::Cancelled | TurnOutcome::Failed { .. } => None,
          };
        }
        if !is_active {
          return None;
        }
        return Some(ServerEvent::TurnFinished {
          correlation,
          outcome,
        });
      }
      ServerEvent::Error {
        request_id,
        session_id,
        message,
      } => {
        if let Some(request_id) = request_id {
          self.remove_pending(request_id);
        }
        if self.creating_session && self.active_session.is_none() {
          self.creating_session = false;
        }
        return Some(ServerEvent::Error {
          request_id,
          session_id,
          message,
        });
      }
      ServerEvent::SessionStopped { session_id } => {
        let was_active = self.is_active(session_id);
        self
          .pending
          .retain(|_, pending| pending.session_id != Some(session_id));
        self
          .approvals
          .retain(|approval| approval.session_id != session_id);
        if !was_active {
          return None;
        }
        self.active_session = None;
        self.active_turn = None;
        self.creating_session = false;
        self.awaiting_usage = None;
        self.queued_messages.clear();
        return Some(ServerEvent::SessionStopped { session_id });
      }
      ServerEvent::Exited => return Some(ServerEvent::Exited),
    }
  }
}
