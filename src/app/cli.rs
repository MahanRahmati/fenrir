use std::io::Write;

use tokio_util::sync::CancellationToken;

use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::app::{App, AppEvent, EventSink};
use crate::input::confirm_with;
use crate::input::errors::InputError;
use crate::input::read_from_stdin;
use crate::persona_manager::Persona;
use crate::session_manager::session::Session;
use crate::slash_commands::SlashCommand;
use crate::utils::datetime::format_timestamp;
use crate::utils::usage::format_usage;

struct CliSink {
  stdout: std::io::Stdout,
  reasoning_open: bool,
  reasoning_ends_with_newline: bool,
}

#[async_trait::async_trait]
impl EventSink for CliSink {
  fn on_event(&mut self, event: AppEvent) {
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
      AppEvent::ToolCall { name, arguments } => {
        if self.reasoning_open {
          self.reasoning_open = false;
          if !self.reasoning_ends_with_newline {
            println!();
          }
        }
        println!("[tool] {name}({arguments})");
        self.stdout.flush().ok();
      }
      AppEvent::ToolResult { result, error } => {
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
      AppEvent::Error { message } => {
        println!("\nError: {message}");
      }
    }
  }

  fn confirm(&mut self, message: &str) -> bool {
    return confirm_with(&format!("{message} (y/N): "));
  }

  async fn read_input(&mut self) -> RuntimeResult<Option<String>> {
    return match read_from_stdin() {
      Ok(input) => Ok(Some(input)),
      Err(InputError::Eof) | Err(InputError::EmptyInput) => Ok(None),
      Err(e) => Err(RuntimeError::Input(e.to_string())),
    };
  }

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
    current: Option<i64>,
  ) -> RuntimeResult<Option<i64>> {
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
  ) -> RuntimeResult<Option<String>> {
    let persona_name = handle_personas(personas, &current)?;
    return Ok(persona_name);
  }

  fn on_persona_selection(&mut self, name: String) {
    println!("Persona set to: {name}");
  }
}

impl App {
  pub async fn run(&mut self) -> RuntimeResult<String> {
    let mut sink = CliSink {
      stdout: std::io::stdout(),
      reasoning_open: false,
      reasoning_ends_with_newline: false,
    };

    let cancel = CancellationToken::new();
    return self.harness_loop(&mut sink, &cancel).await;
  }
}

fn print_indexed_list(lines: &[String]) {
  for (index, line) in lines.iter().enumerate() {
    println!("[{index}] {line}");
  }
}

fn handle_sessions(
  sessions: Vec<Session>,
  current: Option<i64>,
) -> RuntimeResult<Option<i64>> {
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
) -> RuntimeResult<Option<String>> {
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

fn read_index(count: usize, name: &str) -> RuntimeResult<Option<usize>> {
  let input = match read_from_stdin() {
    Ok(input) => input,
    Err(InputError::Eof) => return Ok(None),
    Err(InputError::EmptyInput) => return Ok(None),
    Err(e) => return Err(RuntimeError::Input(e.to_string())),
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
  for msg in &session.messages {
    if msg.role == "system" {
      continue;
    }
    if let Some(calls) = &msg.tool_calls {
      for call in calls {
        println!("[tool] {}({})", call.function.name, call.function.arguments);
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
