use std::collections::{HashSet, VecDeque};
use std::io::{BufRead, Write};

use anyhow::{Context, Result};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc::{self, Receiver, Sender};
use uuid::Uuid;

use fenrir_core::command::Command;
use fenrir_core::engine::protocol::{
  CommandInfo, PersonaInfo, ServerEvent, SessionSummary, SessionView,
};
use fenrir_core::provider_manager::Model;
use fenrir_core::session_manager::Message;
use fenrir_core::tool_manager::SUBAGENT_TOOL_NAME;
use fenrir_core::tool_manager::task_summary;
use fenrir_core::tool_manager::tool::parse_tool_params;
use fenrir_core::utils::datetime::format_timestamp;
use fenrir_core::utils::usage::format_usage;

use fenrir_core::client::ApiClient;

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
  client: ApiClient,
  input: Receiver<Option<String>>,
  input_queue: VecDeque<String>,
  eof: bool,
  mode: Mode,
  new_session_request: Option<u64>,
  pending_review_request: Option<u64>,
  pending: HashSet<u64>,
  turn_requests: HashSet<u64>,
  current_persona: Option<String>,
  current_model: Option<Model>,
  reasoning_open: bool,
  reasoning_ends_with_newline: bool,
}

pub async fn run(base_url: &str) -> Result<()> {
  let cwd = std::env::current_dir()
    .map(|path| path.display().to_string())
    .unwrap_or_default();
  let client = ApiClient::connect(base_url, &cwd).await?;
  let (current_persona, current_model) = match client.defaults().await {
    Ok(defaults) => (Some(defaults.persona), defaults.model),
    Err(_) => (None, None),
  };
  let (input_tx, input_rx) = mpsc::channel(32);
  spawn_input_task(input_tx);
  let mut adapter = Adapter {
    client,
    input: input_rx,
    input_queue: VecDeque::new(),
    eof: false,
    mode: Mode::Idle,
    new_session_request: None,
    pending_review_request: None,
    pending: HashSet::new(),
    turn_requests: HashSet::new(),
    current_persona,
    current_model,
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
        event = self.client.recv() => {
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
          if self.is_busy() {
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
        && !self.is_busy()
        && self.client.state().awaiting_usage().is_none()
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
        && (self.is_busy() || self.client.state().awaiting_usage().is_some())
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
          if let Err(error) = self
            .client
            .approve_tool(session_id, turn_id, tool_call_id, is_yes(&line))
            .await
          {
            print_error(&error);
          }
          false
        }
        Mode::Sessions { sessions } => {
          if let Some(index) = parse_index(&line, sessions.len(), "session")
            && let Err(error) = self.load_session(sessions[index].id).await
          {
            print_error(&error);
          }
          false
        }
        Mode::Personas { personas } => {
          if let Some(index) = parse_index(&line, personas.len(), "persona")
            && let Err(error) =
              self.select_persona(personas[index].name.clone()).await
          {
            print_error(&error);
          }
          false
        }
        Mode::Models { models } => {
          if let Some(index) = parse_index(&line, models.len(), "model")
            && let Err(error) = self.select_model(models[index].clone()).await
          {
            print_error(&error);
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
        let request_id = self.client.send_prompt(line.to_string()).await?;
        self.add_pending(request_id, true);
        return Ok(false);
      }
      Err(e) => {
        print_error(&e);
        return Ok(false);
      }
    };

    if command == Command::Exit {
      self.shutdown().await?;
      return Ok(true);
    }

    let result = match command {
      Command::New => self.create_new_session().await,
      Command::Sessions => self.list_sessions().await,
      Command::Personas => self.list_personas().await,
      Command::Models => self.list_models().await,
      Command::Compact => self.compact_session().await,
      Command::Review => self.review_session().await,
      Command::Help => self.show_help().await,
      Command::Exit => Ok(()),
    };
    if let Err(error) = result {
      print_error(&error);
    }
    return Ok(false);
  }

  async fn handle_event(&mut self, event: ServerEvent) -> Result<bool> {
    match event {
      ServerEvent::Connected { .. } => {}
      ServerEvent::SessionCreated {
        request_id,
        session,
      } => {
        self.complete_event_request(request_id);
        self.current_persona = Some(session.persona.clone());
        self.current_model.clone_from(&session.model);
        if self.new_session_request == Some(request_id) {
          self.new_session_request = None;
          println!("Persona set to: {}", session.persona);
        }
        if self.pending_review_request == Some(request_id) {
          self.pending_review_request = None;
          match self.client.review(session.summary.id).await {
            Ok(review_request) => self.add_pending(review_request, true),
            Err(error) => print_error(&error),
          }
        }
      }
      ServerEvent::SessionLoaded {
        request_id,
        session,
      } => {
        self.complete_event_request(request_id);
        self.current_persona = Some(session.persona.clone());
        self.current_model.clone_from(&session.model);
        print_session(&session);
      }
      ServerEvent::SessionsListed { request_id, .. }
      | ServerEvent::PersonasListed { request_id, .. }
      | ServerEvent::ModelsListed { request_id, .. }
      | ServerEvent::CommandCatalog { request_id, .. } => {
        self.complete_event_request(request_id);
      }
      ServerEvent::PersonaUpdated {
        request_id,
        persona,
        ..
      } => {
        self.complete_event_request(request_id);
        self.current_persona = Some(persona.clone());
        println!("Persona set to: {persona}");
      }
      ServerEvent::ModelUpdated {
        request_id, model, ..
      } => {
        self.complete_event_request(request_id);
        self.current_model = Some(model.clone());
        println!("Model set to: {}", model.label());
      }
      ServerEvent::UserMessage { .. } => {}
      ServerEvent::TurnStarted { .. } => {
        self.complete_turn_requests();
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
        usage,
        context_window,
        ..
      } => {
        let usage = usage
          .and_then(|usage| format_usage(usage.prompt_tokens, context_window));
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
      ServerEvent::TurnFinished { .. } => {
        println!();
        flush();
      }
      ServerEvent::Retrying {
        attempt,
        max_attempts,
        delay_seconds,
        reason,
        ..
      } => {
        println!(
          "\n[retry] attempt {attempt}/{max_attempts} in {delay_seconds}s: {reason}"
        );
        flush();
      }
      ServerEvent::Error {
        request_id,
        message,
        ..
      } => {
        if let Some(request_id) = request_id {
          self.complete_pending(request_id);
          if self.new_session_request == Some(request_id) {
            self.new_session_request = None;
          }
          if self.pending_review_request == Some(request_id) {
            self.pending_review_request = None;
          }
        }
        println!("\nError: {message}");
        flush();
      }
      ServerEvent::SessionStopped { .. } => {
        self.pending.clear();
        self.turn_requests.clear();
        self.new_session_request = None;
        self.pending_review_request = None;
      }
      ServerEvent::Exited => return Ok(true),
    }
    return Ok(false);
  }

  async fn create_new_session(&mut self) -> Result<()> {
    let request_id = self.client.create_session(None, None).await?;
    self.add_pending(request_id, false);
    self.new_session_request = Some(request_id);
    return Ok(());
  }

  async fn list_sessions(&mut self) -> Result<()> {
    let sessions = self.client.sessions().await?;
    self.open_sessions(sessions);
    return Ok(());
  }

  async fn list_personas(&mut self) -> Result<()> {
    let personas = self.client.personas().await?;
    let current = self.current_persona.clone();
    self.open_personas(personas, current);
    return Ok(());
  }

  async fn list_models(&mut self) -> Result<()> {
    let models = self.client.models().await?;
    let current = self.current_model.clone();
    self.open_models(models, current);
    return Ok(());
  }

  async fn compact_session(&mut self) -> Result<()> {
    let request_id = self.client.compact().await?;
    self.add_pending(request_id, true);
    return Ok(());
  }

  async fn review_session(&mut self) -> Result<()> {
    match self.client.state().visible_session() {
      Some(session_id) => {
        let request_id = self.client.review(session_id).await?;
        self.add_pending(request_id, true);
      }
      None => {
        let request_id = self.client.create_session(None, None).await?;
        self.add_pending(request_id, false);
        self.pending_review_request = Some(request_id);
      }
    }
    return Ok(());
  }

  async fn show_help(&mut self) -> Result<()> {
    let session_id = self.client.state().visible_session();
    let commands = self.client.commands(session_id).await?;
    print_commands(&commands);
    return Ok(());
  }

  async fn load_session(&mut self, session_id: Uuid) -> Result<()> {
    let request_id = self.client.load_session(session_id).await?;
    self.add_pending(request_id, false);
    return Ok(());
  }

  async fn select_persona(&mut self, name: String) -> Result<()> {
    let session_id = self.client.state().visible_session();
    let request_id = self.client.set_persona_for(session_id, name).await?;
    self.add_pending(request_id, false);
    return Ok(());
  }

  async fn select_model(&mut self, model: Model) -> Result<()> {
    let session_id = self.client.state().visible_session();
    let request_id = self.client.set_model_for(session_id, model).await?;
    self.add_pending(request_id, false);
    return Ok(());
  }

  fn add_pending(&mut self, request_id: u64, starts_turn: bool) {
    self.pending.insert(request_id);
    if starts_turn {
      self.turn_requests.insert(request_id);
    }
  }

  fn complete_event_request(&mut self, request_id: u64) {
    if self.turn_requests.contains(&request_id) {
      return;
    }
    self.complete_pending(request_id);
  }

  fn complete_pending(&mut self, request_id: u64) {
    self.pending.remove(&request_id);
    self.turn_requests.remove(&request_id);
  }

  fn complete_turn_requests(&mut self) {
    for request_id in self.turn_requests.drain() {
      self.pending.remove(&request_id);
    }
  }

  fn is_busy(&self) -> bool {
    return self.client.state().is_busy() || !self.pending.is_empty();
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
    let active_session = self.client.state().visible_session();
    let lines: Vec<String> = sessions
      .iter()
      .map(|session| {
        let marker = match active_session {
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
    println!("\n[cancelling]");
    flush();
    self.client.cancel(None).await?;
    return Ok(());
  }

  async fn shutdown(&mut self) -> Result<()> {
    let _ = self.client.disconnect().await;
    return Ok(());
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

fn print_error(error: &anyhow::Error) {
  println!("\nError: {error}");
  flush();
}

fn flush() {
  let _ = std::io::stdout().flush();
}
