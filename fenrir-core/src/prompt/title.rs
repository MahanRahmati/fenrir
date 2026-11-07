use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::engine::protocol::{Correlation, ServerEvent, SessionEvent};
use crate::engine::repo_actor::RepoHandle;
use crate::mentions::strip_mentions;
use crate::persona_manager::TITLES_PERSONA;
use crate::prompt::{PromptExecutor, PromptKind, PromptSpec, system};
use crate::provider_manager::{Model, ProviderManager};
use crate::session_manager::Message;
use crate::session_manager::session::Session;
use crate::vlog;

pub fn build(
  config: &Config,
  cwd: &str,
  model: &Model,
  first_message: Message,
) -> PromptSpec {
  return PromptSpec {
    kind: PromptKind::Title,
    model: model.clone(),
    system: system::build(config, &TITLES_PERSONA, cwd),
    messages: vec![first_message],
    tools: Vec::new(),
    stream: false,
    emit_tokens: false,
  };
}

pub fn spawn_if_needed(
  session: &mut Session,
  config: &Config,
  repo: &RepoHandle,
  events: &Sender<SessionEvent>,
) {
  if session.title.is_some() || session.title_pending || session.is_subagent() {
    return;
  }
  let first_message = session
    .messages
    .iter()
    .find(|message| message.role == "user")
    .map(|message| strip_mentions(&message.content))
    .unwrap_or_default();
  if first_message.is_empty() {
    return;
  }
  let Some(model) = session.model.clone() else {
    return;
  };

  session.title_pending = true;
  let session_id = session.id;
  let cwd = session.cwd.clone();
  let config = config.clone();
  let repo = repo.clone();
  let events = events.clone();

  tokio::spawn(async move {
    let now = chrono::Utc::now().timestamp();
    let message = Message {
      id: Uuid::now_v7(),
      msg_type: None,
      role: "user".to_string(),
      content: first_message,
      tool_call_id: None,
      tool_calls: None,
      timestamp: now,
    };
    let providers = ProviderManager::new(config.clone());
    let spec = build(&config, &cwd, &model, message);
    let correlation = Correlation {
      session_id,
      turn_id: Uuid::nil(),
    };
    let cancel = CancellationToken::new();
    let output = PromptExecutor::new(&providers, &config)
      .once(spec, correlation, &events, &cancel)
      .await;
    let output = match output {
      Ok(output) => output,
      Err(e) => {
        vlog!("Failed to generate session title: {e}");
        return;
      }
    };

    let Some(content) = output.content else {
      return;
    };
    let title = content
      .trim()
      .trim_matches(|c| c == '"' || c == '\'' || c == '`')
      .to_string();
    if title.is_empty() {
      return;
    }

    if let Err(e) = repo.update_title(session_id, &title).await {
      vlog!("Failed to persist session title: {e}");
      return;
    }
    vlog!("Generated session title: {title}");
    let _ = events
      .send(SessionEvent::Event(ServerEvent::TitleUpdated {
        session_id,
        title,
      }))
      .await;
  });
}
