use std::collections::{HashSet, VecDeque};
use std::io::{BufRead, Write};

use anyhow::{Context, Result};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc::{self, Receiver, Sender};
use uuid::Uuid;

use fenrir_core::command::Command;
use fenrir_core::engine::broker::{BrokerHandle, ClientHandle};
use fenrir_core::engine::protocol::{
  ClientRequest, CommandInfo, PersonaInfo, PromptMode, ServerEvent,
  SessionSummary, SessionView, TurnOutcome,
};
use fenrir_core::persona_manager::DEFAULT_PERSONA;
use fenrir_core::provider_manager::Model;
use fenrir_core::session_manager::Message;
use fenrir_core::tool_manager::SUBAGENT_TOOL_NAME;
use fenrir_core::tool_manager::task_summary;
use fenrir_core::tool_manager::tool::parse_tool_params;
use fenrir_core::utils::datetime::format_timestamp;
use fenrir_core::utils::usage::format_usage;

enum Mode {
  Idle,
  Approval {
    session_id: Uuid,
    turn_id: Uuid,
    tool_call_id: String,
  },
  Sessions {
    sessions: Vec<SessionSummary>,
  },
  Personas {
    personas: Vec<PersonaInfo>,
  },
  Models {
    models: Vec<Model>,
  },
}

struct Adapter {
  client: ClientHandle,
  input: Receiver<Option<String>>,
  input_queue: VecDeque<String>,
  eof: bool,
  mode: Mode,
  active_session: Option<Uuid>,
  active_turn: Option<Uuid>,
  awaiting_usage: Option<Uuid>,
  busy: bool,
  next_request_id: u64,
  pending_request_ids: HashSet<u64>,
  new_session_request: Option<u64>,
  reasoning_open: bool,
  reasoning_ends_with_newline: bool,
}

pub async fn run(broker: &BrokerHandle) -> Result<()> {
  let cwd = std::env::current_dir()
    .map(|path| path.display().to_string())
    .unwrap_or_default();
  let client = broker.connect(&cwd).await?;
  let (input_tx, input_rx) = mpsc::channel(32);
  spawn_input_task(input_tx);
  let mut adapter = Adapter {
    client,
    input: input_rx,
    input_queue: VecDeque::new(),
    eof: false,
    mode: Mode::Idle,
    active_session: None,
    active_turn: None,
    awaiting_usage: None,
    busy: false,
    next_request_id: 1,
    pending_request_ids: HashSet::new(),
    new_session_request: None,
    reasoning_open: false,
    reasoning_ends_with_newline: false,
  };
  return adapter.run().await;
}

impl Adapter {
  async fn run(&mut self) -> Result<()> {
    let mut sigint =
      signal(SignalKind::interrupt()).context("Failed to listen for SIGINT")?;
    let mut sigterm = signal(SignalKind::terminate())
      .context("Failed to listen for SIGTERM")?;

    loop {
      tokio::select! {
        biased;
        event = self.client.events.recv() => {
          match event {
            Some(event) => {
              if self.handle_event(event).await? {
                break;
              }
            }
            None => break,
          }
        }
        line = self.input.recv(), if !self.eof => {
          match line {
            Some(Some(line)) => self.input_queue.push_back(line),
            Some(None) | None => self.eof = true,
          }
        }
        _ = sigint.recv() => {
          if self.active_turn.is_some() || self.busy {
            self.cancel_turn().await?;
          } else {
            self.shutdown().await?;
            break;
          }
        }
        _ = sigterm.recv() => {
          self.shutdown().await?;
          break;
        }
      }

      if self.process_input().await? {
        break;
      }

      if self.eof
        && self.input_queue.is_empty()
        && !self.busy
        && self.active_turn.is_none()
        && self.awaiting_usage.is_none()
        && matches!(self.mode, Mode::Idle)
      {
        self.shutdown().await?;
        break;
      }
    }

    return Ok(());
  }

  async fn process_input(&mut self) -> Result<bool> {
    loop {
      if matches!(self.mode, Mode::Idle)
        && (self.busy
          || self.active_turn.is_some()
          || self.awaiting_usage.is_some())
      {
        return Ok(false);
      }
      let Some(line) = self.input_queue.pop_front() else {
        return Ok(false);
      };
      let mode = std::mem::replace(&mut self.mode, Mode::Idle);
      let exit = match mode {
        Mode::Idle => self.handle_line(&line).await?,
        Mode::Approval {
          session_id,
          turn_id,
          tool_call_id,
        } => {
          self
            .client
            .send(ClientRequest::ApproveTool {
              session_id,
              turn_id,
              tool_call_id,
              approved: is_yes(&line),
            })
            .await?;
          false
        }
        Mode::Sessions { sessions } => {
          if let Some(index) = parse_index(&line, sessions.len(), "session") {
            let request_id = self.take_request_id();
            self.pending_request_ids.insert(request_id);
            self.busy = true;
            self
              .client
              .send(ClientRequest::LoadSession {
                request_id,
                session_id: sessions[index].id,
              })
              .await?;
          }
          false
        }
        Mode::Personas { personas } => {
          if let Some(index) = parse_index(&line, personas.len(), "persona") {
            let request_id = self.take_request_id();
            self.pending_request_ids.insert(request_id);
            self.busy = true;
            self
              .client
              .send(ClientRequest::SetPersona {
                request_id,
                session_id: self.active_session,
                name: personas[index].name.clone(),
              })
              .await?;
          }
          false
        }
        Mode::Models { models } => {
          if let Some(index) = parse_index(&line, models.len(), "model") {
            let request_id = self.take_request_id();
            self.pending_request_ids.insert(request_id);
            self.busy = true;
            self
              .client
              .send(ClientRequest::SetModel {
                request_id,
                session_id: self.active_session,
                model: models[index].clone(),
              })
              .await?;
          }
          false
        }
      };
      if exit {
        return Ok(true);
      }
    }
  }

  async fn handle_line(&mut self, line: &str) -> Result<bool> {
    if line.is_empty() {
      return Ok(false);
    }

    let command = match Command::parse(line) {
      Ok(Some(command)) => command,
      Ok(None) => {
        let request_id = self.take_request_id();
        self.pending_request_ids.insert(request_id);
        self.busy = true;
        self
          .client
          .send(ClientRequest::SendPrompt {
            request_id,
            session_id: self.active_session,
            mode: PromptMode::Normal,
            text: Some(line.to_string()),
          })
          .await?;
        return Ok(false);
      }
      Err(e) => {
        println!("\nError: {e}");
        flush();
        return Ok(false);
      }
    };

    if command == Command::Exit {
      self.shutdown().await?;
      return Ok(true);
    }

    let request_id = self.take_request_id();
    self.pending_request_ids.insert(request_id);
    self.busy = true;
    if command == Command::New {
      self.new_session_request = Some(request_id);
    }
    self
      .client
      .send(ClientRequest::RunCommand {
        request_id,
        session_id: self.active_session,
        command,
      })
      .await?;
    return Ok(false);
  }

  async fn handle_event(&mut self, event: ServerEvent) -> Result<bool> {
    match event {
      ServerEvent::Connected { .. } => {}
      ServerEvent::SessionCreated {
        request_id,
        session,
      } => {
        self.pending_request_ids.remove(&request_id);
        self.active_session = Some(session.summary.id);
        if self.new_session_request == Some(request_id) {
          self.new_session_request = None;
          self.busy = false;
          println!("Persona set to: {}", DEFAULT_PERSONA.name());
        }
      }
      ServerEvent::SessionLoaded {
        request_id,
        session,
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        self.active_session = Some(session.summary.id);
        print_session(&session);
      }
      ServerEvent::SessionsListed {
        request_id,
        sessions,
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        self.open_sessions(sessions);
      }
      ServerEvent::PersonasListed {
        request_id,
        personas,
        current,
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        self.open_personas(personas, current);
      }
      ServerEvent::ModelsListed {
        request_id,
        models,
        current,
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        self.open_models(models, current);
      }
      ServerEvent::PersonaUpdated {
        request_id,
        persona,
        ..
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        println!("Persona set to: {persona}");
      }
      ServerEvent::ModelUpdated {
        request_id, model, ..
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        println!("Model set to: {}", model.label());
      }
      ServerEvent::CommandCatalog {
        request_id,
        commands,
      } => {
        self.pending_request_ids.remove(&request_id);
        self.busy = false;
        print_commands(&commands);
      }
      ServerEvent::UserMessage { .. } => {}
      ServerEvent::TurnStarted { correlation, .. } => {
        self.busy = true;
        self.active_turn = Some(correlation.turn_id);
        println!("[Processing...]");
        flush();
      }
      ServerEvent::Token {
        text, reasoning, ..
      } => {
        self.render_token(&text, reasoning);
      }
      ServerEvent::ToolCall {
        name, arguments, ..
      } => {
        self.close_reasoning();
        if name == SUBAGENT_TOOL_NAME {
          let task = parse_tool_params(&arguments)
            .remove("task")
            .unwrap_or_default();
          println!("[subagent] {}", task_summary(&task));
        } else {
          println!("[tool] {name}({arguments})");
        }
        flush();
      }
      ServerEvent::ToolApprovalRequest {
        correlation,
        tool_call_id,
        message,
        ..
      } => {
        self.close_reasoning();
        print!("{message} (y/N): ");
        flush();
        self.mode = Mode::Approval {
          session_id: correlation.session_id,
          turn_id: correlation.turn_id,
          tool_call_id,
        };
      }
      ServerEvent::ToolApprovalResult { .. } => {}
      ServerEvent::ToolResult { result, error, .. } => {
        if error {
          println!("[error] {result}");
        } else {
          println!("[result] {result}");
        }
        flush();
      }
      ServerEvent::Usage {
        correlation, usage, ..
      } => {
        if self.awaiting_usage == Some(correlation.turn_id) {
          self.awaiting_usage = None;
        }
        let usage =
          usage.and_then(|usage| format_usage(usage.prompt_tokens, None));
        if let Some(usage) = usage {
          println!("\n[usage] {usage}");
        }
      }
      ServerEvent::Subagents { .. } => {}
      ServerEvent::Compacted { summary, .. } => {
        println!("\n[Compaction Summary]\n");
        println!("{summary}\n");
      }
      ServerEvent::Prediction { .. } => {}
      ServerEvent::TitleUpdated { .. } => {}
      ServerEvent::QueueUpdated { .. } => {}
      ServerEvent::TurnFinished {
        correlation,
        outcome,
      } => {
        if self.active_turn == Some(correlation.turn_id) {
          self.active_turn = None;
        }
        self.awaiting_usage = match outcome {
          TurnOutcome::Completed { .. } => Some(correlation.turn_id),
          TurnOutcome::Cancelled | TurnOutcome::Failed { .. } => None,
        };
        self.busy = false;
        println!();
        flush();
      }
      ServerEvent::Error {
        request_id,
        message,
        ..
      } => {
        if let Some(request_id) = request_id {
          self.pending_request_ids.remove(&request_id);
          if self.new_session_request == Some(request_id) {
            self.new_session_request = None;
          }
          if self.active_turn.is_none() {
            self.busy = false;
          }
        }
        println!("\nError: {message}");
        flush();
      }
      ServerEvent::SessionStopped { session_id } => {
        if self.active_session == Some(session_id) {
          self.active_session = None;
          self.awaiting_usage = None;
        }
      }
      ServerEvent::Exited => return Ok(true),
    }
    return Ok(false);
  }

  fn render_token(&mut self, text: &str, reasoning: bool) {
    if reasoning {
      if !self.reasoning_open {
        print!("Reasoning: ");
        self.reasoning_open = true;
      }
      self.reasoning_ends_with_newline = text.ends_with('\n');
      print!("{text}");
    } else {
      self.close_reasoning();
      print!("{text}");
    }
    flush();
  }

  fn close_reasoning(&mut self) {
    if self.reasoning_open {
      self.reasoning_open = false;
      if !self.reasoning_ends_with_newline {
        println!();
      }
    }
  }

  fn open_sessions(&mut self, sessions: Vec<SessionSummary>) {
    if sessions.is_empty() {
      println!("No sessions found.");
      return;
    }
    let lines: Vec<String> = sessions
      .iter()
      .map(|session| {
        let marker = match self.active_session {
          Some(id) if id == session.id => "*",
          _ => " ",
        };
        let title = session.title.as_deref().unwrap_or("Untitled Session");
        format!(
          "{marker}{title}  {}",
          format_timestamp(session.last_interaction)
        )
      })
      .collect();
    print_indexed_list(&lines);
    println!("Enter session index to switch to (or empty to skip): ");
    flush();
    self.mode = Mode::Sessions { sessions };
  }

  fn open_personas(
    &mut self,
    personas: Vec<PersonaInfo>,
    current: Option<String>,
  ) {
    let lines: Vec<String> = personas
      .iter()
      .map(|persona| {
        let marker = match current.as_deref() {
          Some(name) if name == persona.name => "*",
          _ => " ",
        };
        format!("{marker}{} - {}", persona.name, persona.description)
      })
      .collect();
    print_indexed_list(&lines);
    println!("Enter persona index to select (or empty to skip): ");
    flush();
    self.mode = Mode::Personas { personas };
  }

  fn open_models(&mut self, models: Vec<Model>, current: Option<Model>) {
    if models.is_empty() {
      println!("No models found.");
      return;
    }
    let lines: Vec<String> = models
      .iter()
      .map(|model| {
        let marker = match &current {
          Some(current) if current == model => "*",
          _ => " ",
        };
        format!("{marker}{} ({})", model.label(), model.provider)
      })
      .collect();
    print_indexed_list(&lines);
    println!("Enter model index to switch to (or empty to skip): ");
    flush();
    self.mode = Mode::Models { models };
  }

  async fn cancel_turn(&mut self) -> Result<()> {
    let Some(session_id) = self.active_session else {
      return Ok(());
    };
    println!("\n[cancelling]");
    flush();
    self
      .client
      .send(ClientRequest::Cancel {
        session_id,
        turn_id: None,
      })
      .await?;
    return Ok(());
  }

  async fn shutdown(&mut self) -> Result<()> {
    let _ = self.client.send(ClientRequest::Shutdown).await;
    return Ok(());
  }

  fn take_request_id(&mut self) -> u64 {
    let request_id = self.next_request_id;
    self.next_request_id += 1;
    return request_id;
  }
}

fn spawn_input_task(tx: Sender<Option<String>>) {
  std::thread::spawn(move || {
    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    loop {
      let mut line = String::new();
      match stdin.read_line(&mut line) {
        Ok(0) => {
          let _ = tx.blocking_send(None);
          break;
        }
        Ok(_) => {
          if tx.blocking_send(Some(line.trim().to_string())).is_err() {
            break;
          }
        }
        Err(_) => {
          let _ = tx.blocking_send(None);
          break;
        }
      }
    }
  });
}

fn print_session(session: &SessionView) {
  println!(
    "--- {} ---",
    session
      .summary
      .title
      .as_deref()
      .unwrap_or("Untitled Session")
  );
  print_session_messages(&session.messages);
}

fn print_commands(commands: &[CommandInfo]) {
  println!("Available commands:");
  for command in commands {
    println!("  {}: {}", command.name, command.description);
  }
}

fn print_indexed_list(lines: &[String]) {
  for (index, line) in lines.iter().enumerate() {
    println!("[{index}] {line}");
  }
}

fn print_session_messages(messages: &[Message]) {
  let mut subagent_calls: HashSet<String> = HashSet::new();
  for message in messages {
    if message.role == "system" {
      continue;
    }
    if let Some(calls) = &message.tool_calls {
      for call in calls {
        if call.function.name == SUBAGENT_TOOL_NAME {
          let task = parse_tool_params(&call.function.arguments)
            .remove("task")
            .unwrap_or_default();
          println!("[subagent] {}", task_summary(&task));
          subagent_calls.insert(call.id.clone());
        } else {
          println!(
            "[tool] {}({})",
            call.function.name, call.function.arguments
          );
        }
      }
      continue;
    }
    match message.role.as_str() {
      "user" | "assistant" => println!("{}", message.content),
      "reasoning" => {
        print!("Reasoning: {}", message.content);
        if !message.content.ends_with('\n') {
          println!();
        }
      }
      "tool" => {
        if let Some(call_id) = &message.tool_call_id
          && subagent_calls.contains(call_id)
        {
          continue;
        }
        let content = message
          .content
          .strip_prefix("Tool result: ")
          .unwrap_or(&message.content);
        if content.starts_with("Tool error: ") {
          println!("[error] {content}");
        } else {
          println!("[result] {content}");
        }
      }
      _ => {}
    }
  }
}

fn parse_index(input: &str, count: usize, name: &str) -> Option<usize> {
  if input.is_empty() {
    return None;
  }
  let index = match input.parse::<usize>() {
    Ok(index) => index,
    Err(_) => {
      println!("Invalid {name} index: {input}");
      return None;
    }
  };
  if index >= count {
    println!("No {name} at index {index}.");
    return None;
  }
  return Some(index);
}

fn is_yes(input: &str) -> bool {
  let answer = input.trim().to_lowercase();
  return answer == "y" || answer == "yes";
}

fn flush() {
  let _ = std::io::stdout().flush();
}
