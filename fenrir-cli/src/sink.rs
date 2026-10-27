use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use fenrir_core::agent_manager::AgentManager;
use fenrir_core::app::{App, AppEvent, EventSink};
use fenrir_core::persona_manager::Persona;
use fenrir_core::provider_manager::Model;
use fenrir_core::session_manager::session::Session;
use fenrir_core::slash_commands::SlashCommand;
use fenrir_core::tool_manager::SUBAGENT_TOOL_NAME;
use fenrir_core::tool_manager::task_summary;
use fenrir_core::tool_manager::tool::parse_tool_params;
use fenrir_core::utils::datetime::format_timestamp;
use fenrir_core::utils::usage::format_usage;

use crate::input::confirm_with;
use crate::input::read_from_stdin;

struct CliSink {
  stdout: std::io::Stdout,
  reasoning_open: bool,
  reasoning_ends_with_newline: bool,
}

#[async_trait::async_trait]
impl EventSink for CliSink {
  async fn on_event(&mut self, event: AppEvent) {
    match event {
      AppEvent::UserMessage { .. } => {}
      AppEvent::Token { text, reasoning } => {
        if reasoning {
          if !self.reasoning_open {
            print!("Reasoning: ");
            self.reasoning_open = true;
          }
          self.reasoning_ends_with_newline = text.ends_with('\n');
          print!("{text}");
        } else {
          if self.reasoning_open {
            self.reasoning_open = false;
            if !self.reasoning_ends_with_newline {
              println!();
            }
          }
          print!("{text}");
        }
        self.stdout.flush().ok();
      }
      AppEvent::ToolCall {
        id: _,
        name,
        arguments,
      } => {
        if self.reasoning_open {
          self.reasoning_open = false;
          if !self.reasoning_ends_with_newline {
            println!();
          }
        }
        println!("[tool] {name}({arguments})");
        self.stdout.flush().ok();
      }
      AppEvent::ToolApprovalRequest { .. } => {}
      AppEvent::ToolResult {
        id: _,
        result,
        error,
      } => {
        if error {
          println!("[error] {result}");
        } else {
          println!("[result] {result}");
        }
        self.stdout.flush().ok();
      }
      AppEvent::Processing { processing } => {
        if processing {
          println!("[Processing...]");
          self.stdout.flush().ok();
        } else {
          println!();
          self.stdout.flush().ok();
        }
      }
      AppEvent::Usage { usage } => {
        let usage = usage.and_then(|u| format_usage(u.prompt_tokens));
        if let Some(usage) = usage {
          println!("\n[usage] {usage}",);
        }
      }
      AppEvent::SubagentStarted { task } => {
        if self.reasoning_open {
          self.reasoning_open = false;
          if !self.reasoning_ends_with_newline {
            println!();
          }
        }
        println!("[subagent] {}", task_summary(&task));
        self.stdout.flush().ok();
      }
      AppEvent::Subagents { .. } => {}
      AppEvent::Compacted { summary } => {
        println!("\n[Compaction Summary]\n");
        println!("{summary}\n");
      }
      AppEvent::Error { message } => {
        println!("\nError: {message}");
      }
      AppEvent::ToolConfirmation { message, reply } => {
        let confirmed = confirm_with(&format!("{message} (y/N): "));
        reply.send(confirmed).ok();
      }
    }
  }

  async fn read_input(&mut self) -> Result<Option<String>> {
    return read_from_stdin();
  }

  async fn poll_events(&mut self) {}

  fn on_exit(&mut self) {}

  fn on_help(&mut self, commands: Vec<SlashCommand>) {
    println!("Available commands:");
    for cmd in commands {
      println!("  {}: {}", cmd.name(), cmd.description());
    }
  }

  fn on_sessions(
    &mut self,
    sessions: Vec<Session>,
    current: Option<Uuid>,
  ) -> Result<Option<Uuid>> {
    let session_id = handle_sessions(sessions, current)?;
    return Ok(session_id);
  }

  fn on_session_selection(&mut self, session: Session) {
    println!(
      "--- {} ---",
      session.title.as_deref().unwrap_or("Untitled Session")
    );
    print_session_messages(&session);
  }

  fn on_personas(
    &mut self,
    personas: Vec<Persona>,
    current: String,
  ) -> Result<Option<String>> {
    let persona_name = handle_personas(personas, &current)?;
    return Ok(persona_name);
  }

  fn on_persona_selection(&mut self, name: String) {
    println!("Persona set to: {name}");
  }

  fn on_models(
    &mut self,
    models: Vec<Model>,
    current: Option<Model>,
  ) -> Result<Option<Model>> {
    let model = handle_models(models, current)?;
    return Ok(model);
  }

  fn on_model_selection(&mut self, model: Model) {
    println!("Model set to: {}", model.label());
  }

  fn set_cancel_token(&mut self, _token: CancellationToken) {}
}

pub async fn run(app: &Arc<Mutex<App>>) -> Result<String> {
  let mut sink = CliSink {
    stdout: std::io::stdout(),
    reasoning_open: false,
    reasoning_ends_with_newline: false,
  };
  return AgentManager::harness_loop(app, &mut sink).await;
}

fn print_indexed_list(lines: &[String]) {
  for (index, line) in lines.iter().enumerate() {
    println!("[{index}] {line}");
  }
}

fn handle_sessions(
  sessions: Vec<Session>,
  current: Option<Uuid>,
) -> Result<Option<Uuid>> {
  if sessions.is_empty() {
    println!("No sessions found.");
    return Ok(None);
  }
  let lines: Vec<String> = sessions
    .iter()
    .map(|session| {
      let marker = match current {
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
  let Some(index) = read_index(sessions.len(), "session")? else {
    return Ok(None);
  };
  let session_id = sessions[index].id;
  return Ok(Some(session_id));
}

fn handle_personas(
  personas: Vec<Persona>,
  current: &str,
) -> Result<Option<String>> {
  let lines: Vec<String> = personas
    .iter()
    .map(|p| {
      let marker = if p.name() == current { "*" } else { " " };
      format!("{marker}{} - {}", p.name(), p.description())
    })
    .collect();
  print_indexed_list(&lines);
  println!("Enter persona index to select (or empty to skip): ");
  let Some(index) = read_index(personas.len(), "persona")? else {
    return Ok(None);
  };
  let persona = &personas[index];
  return Ok(Some(persona.name().to_string()));
}

fn handle_models(
  models: Vec<Model>,
  current: Option<Model>,
) -> Result<Option<Model>> {
  if models.is_empty() {
    println!("No models found.");
    return Ok(None);
  }
  let lines: Vec<String> = models
    .iter()
    .map(|m| {
      let marker = match &current {
        Some(c) if c == m => "*",
        _ => " ",
      };
      format!("{marker}{} ({})", m.label(), m.provider)
    })
    .collect();
  print_indexed_list(&lines);
  println!("Enter model index to switch to (or empty to skip): ");
  let Some(index) = read_index(models.len(), "model")? else {
    return Ok(None);
  };
  return Ok(Some(models[index].clone()));
}

fn read_index(count: usize, name: &str) -> Result<Option<usize>> {
  let Some(input) = read_from_stdin()? else {
    return Ok(None);
  };
  let index = match input.trim().parse::<usize>() {
    Ok(index) => index,
    Err(_) => {
      println!("Invalid {name} index: {input}");
      return Ok(None);
    }
  };
  if index >= count {
    println!("No {name} at index {index}.");
    return Ok(None);
  }
  return Ok(Some(index));
}

fn print_session_messages(session: &Session) {
  let mut subagent_calls: HashSet<String> = HashSet::new();
  for msg in &session.messages {
    if msg.role == "system" {
      continue;
    }
    if let Some(calls) = &msg.tool_calls {
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
    match msg.role.as_str() {
      "user" | "assistant" => println!("{}", msg.content),
      "reasoning" => {
        print!("Reasoning: {}", msg.content);
        if !msg.content.ends_with('\n') {
          println!();
        }
      }
      "tool" => {
        if let Some(call_id) = &msg.tool_call_id
          && subagent_calls.contains(call_id)
        {
          continue;
        }
        let content = msg
          .content
          .strip_prefix("Tool result: ")
          .unwrap_or(&msg.content);
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
