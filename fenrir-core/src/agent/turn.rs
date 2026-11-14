use anyhow::{Error, anyhow};
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use crate::agent::{AgentContext, run_agent_loop};
use crate::engine::jobs;
use crate::engine::protocol::{
  Correlation, PromptMode, ServerEvent, SessionEvent, TurnOutcome,
};
use crate::persona_manager::REVIEWER_PERSONA;
use crate::prompt::{compaction, review, system};
use crate::session_manager::session::Session;

pub async fn run_turn(
  session: &mut Session,
  turn_id: Uuid,
  request_id: Option<u64>,
  client_id: Uuid,
  mode: PromptMode,
  text: Option<String>,
  context: &AgentContext<'_>,
) -> TurnOutcome {
  let correlation = Correlation {
    session_id: session.id,
    turn_id,
  };

  if let Err(e) = session
    .ensure_system_message_with(context.config, context.repo)
    .await
  {
    return failed(context.events, correlation, e).await;
  }

  let input = match mode {
    PromptMode::Normal => match text {
      Some(text) => text,
      None => {
        return failed(
          context.events,
          correlation,
          anyhow!("Missing prompt text"),
        )
        .await;
      }
    },
    PromptMode::Review => {
      let prompt = review::build(session);
      if let Err(e) = session.persona_manager.set(REVIEWER_PERSONA.name()) {
        return failed(context.events, correlation, e).await;
      }
      session.persona = session.persona_manager.current().name().to_string();
      if let Err(e) =
        context.repo.set_persona(session.id, &session.persona).await
      {
        return failed(context.events, correlation, e).await;
      }
      if let Some(model) = context.provider_manager.large_model()
        && session.model.as_ref() != Some(&model)
      {
        if let Err(e) = context.repo.set_model(session.id, &model).await {
          return failed(context.events, correlation, e).await;
        }
        session.model = Some(model.clone());
        if let Some(request_id) = request_id {
          let _ = context
            .events
            .send(SessionEvent::Event(ServerEvent::ModelUpdated {
              request_id,
              session_id: Some(session.id),
              model,
            }))
            .await;
        }
      }
      session.replace_system_message(&system::build(
        context.config,
        session.persona_manager.current(),
        &session.cwd,
      ));
      if let Some(request_id) = request_id {
        let _ = context
          .events
          .send(SessionEvent::Event(ServerEvent::PersonaUpdated {
            request_id,
            session_id: Some(session.id),
            persona: session.persona.clone(),
          }))
          .await;
      }
      prompt
    }
  };

  session.add_user_message(&input);
  if let Err(e) = session.persist_last_message(context.repo).await {
    return failed(context.events, correlation, e).await;
  }
  let _ = context
    .events
    .send(SessionEvent::Event(ServerEvent::UserMessage {
      correlation,
      client_id,
      content: input,
    }))
    .await;

  let result = run_agent_loop(session, context, correlation).await;

  if context.cancel.is_cancelled() {
    return TurnOutcome::Cancelled;
  }

  let assistant = match result {
    Ok(assistant) => assistant,
    Err(e) => return failed(context.events, correlation, e).await,
  };

  jobs::spawn_title(
    session,
    context.config,
    context.secrets,
    context.repo,
    context.events,
  );

  if let Err(e) = compaction::run_if_needed(
    session,
    context.config,
    context.provider_manager,
    context.repo,
    context.events,
    correlation,
    context.cancel,
  )
  .await
    && !context.cancel.is_cancelled()
  {
    let _ = context
      .events
      .send(SessionEvent::Event(ServerEvent::Error {
        request_id: None,
        session_id: Some(session.id),
        message: format!("Auto-compaction failed: {e}"),
      }))
      .await;
  }

  return TurnOutcome::Completed { assistant };
}

pub async fn run_compact(
  session: &mut Session,
  turn_id: Uuid,
  request_id: u64,
  context: &AgentContext<'_>,
) -> TurnOutcome {
  let correlation = Correlation {
    session_id: session.id,
    turn_id,
  };

  match compaction::run_once(
    session,
    context.config,
    context.provider_manager,
    context.repo,
    context.events,
    correlation,
    context.cancel,
  )
  .await
  {
    Ok(summary) => TurnOutcome::Completed { assistant: summary },
    Err(_) if context.cancel.is_cancelled() => TurnOutcome::Cancelled,
    Err(e) => {
      let message = error_chain(&e);
      let _ = context
        .events
        .send(SessionEvent::Event(ServerEvent::Error {
          request_id: Some(request_id),
          session_id: Some(session.id),
          message: format!("Compaction failed: {message}"),
        }))
        .await;
      return TurnOutcome::Failed { message };
    }
  }
}

fn error_chain(error: &Error) -> String {
  let mut parts = vec![error.to_string()];
  let mut source = error.source();
  while let Some(s) = source {
    parts.push(s.to_string());
    source = s.source();
  }
  return parts.join(": ");
}

async fn failed(
  events: &Sender<SessionEvent>,
  correlation: Correlation,
  error: Error,
) -> TurnOutcome {
  let message = error_chain(&error);
  let _ = events
    .send(SessionEvent::Event(ServerEvent::Error {
      request_id: None,
      session_id: Some(correlation.session_id),
      message: message.clone(),
    }))
    .await;
  return TurnOutcome::Failed { message };
}
