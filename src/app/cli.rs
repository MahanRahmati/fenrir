use std::io::Write;

use crate::app::command::{AppCommand, AppCommandResult};
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::app::{App, AppEvent};
use crate::input::confirm_with;
use crate::input::errors::InputError;
use crate::input::read_from_stdin;
use crate::session_manager::session::Session;
use crate::utils::datetime::format_timestamp;
use crate::vlog;

impl App {
  pub async fn run(&mut self) -> RuntimeResult<String> {
    loop {
      if self.shutdown.is_cancelled() {
        vlog!("Shutdown signal received, exiting CLI loop...");
        break;
      }

      let input = match read_from_stdin() {
        Ok(input) => input,
        Err(InputError::Eof) => break,
        Err(InputError::EmptyInput) => continue,
        Err(e) => return Err(RuntimeError::Input(e.to_string())),
      };

      let command = match AppCommand::parse(&input) {
        Ok(Some(command)) => Some(command),
        Ok(None) => None,
        Err(e) => {
          println!("Error: {e}");
          continue;
        }
      };

      if let Some(command) = command {
        match self.handle_command(command).await? {
          AppCommandResult::Exit => break,
          AppCommandResult::Help(commands) => {
            println!("Available commands:");
            for cmd in commands {
              println!("  {}: {}", cmd.name(), cmd.description());
            }
          }
          AppCommandResult::Sessions { sessions, current } => {
            if sessions.is_empty() {
              println!("No sessions found.");
              continue;
            }
            let lines: Vec<String> = sessions
              .iter()
              .map(|session| {
                let marker = match current {
                  Some(id) if id == session.id => "*",
                  _ => " ",
                };
                let title =
                  session.title.as_deref().unwrap_or("Untitled Session");
                format!(
                  "{marker}{title}  {}",
                  format_timestamp(session.last_interaction)
                )
              })
              .collect();
            print_indexed_list(&lines);
            println!("Enter session index to switch to (or empty to skip): ");
            let Some(index) = read_index(sessions.len(), "session")? else {
              continue;
            };
            let session_id = sessions[index].id;
            if let AppCommandResult::Session(session) = self
              .handle_command(AppCommand::LoadSession { id: session_id })
              .await?
            {
              println!(
                "--- {} ---",
                session.title.as_deref().unwrap_or("Untitled Session")
              );
              print_session_messages(&session);
            }
          }
          AppCommandResult::Personas { personas, current } => {
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
              continue;
            };
            let persona = &personas[index];
            self
              .handle_command(AppCommand::SelectPersona {
                name: persona.name().to_string(),
              })
              .await?;
            println!("Persona set to: {}", persona.name());
          }
          AppCommandResult::Persona(_) => {
            println!("Started a new session.");
          }
          _ => {}
        }
        continue;
      }

      self.ensure_session_with_system_message().await?;

      self
        .session_manager
        .add_user_message_to_current_session(&input)
        .await
        .map_err(|e| RuntimeError::Session(e.to_string()))?;

      let mut stdout = std::io::stdout();
      let mut reasoning_open = false;
      let mut reasoning_ends_with_newline = false;
      let handler = |event: AppEvent| -> bool {
        match event {
          AppEvent::Token { text, reasoning } => {
            if reasoning {
              if !reasoning_open {
                print!("Reasoning: ");
                reasoning_open = true;
              }
              reasoning_ends_with_newline = text.ends_with('\n');
              print!("{text}");
            } else {
              if reasoning_open {
                reasoning_open = false;
                if !reasoning_ends_with_newline {
                  println!();
                }
              }
              print!("{text}");
            }
            stdout.flush().ok();
            return true;
          }
          AppEvent::Confirm(msg) => confirm_with(&format!("{msg} (y/N): ")),
          AppEvent::ToolCall { name, arguments } => {
            if reasoning_open {
              reasoning_open = false;
              if !reasoning_ends_with_newline {
                println!();
              }
            }
            println!("[tool] {name}({arguments})");
            stdout.flush().ok();
            return true;
          }
          AppEvent::ToolResult { result, error } => {
            if error {
              println!("[error] {result}");
            } else {
              println!("[result] {result}");
            }
            stdout.flush().ok();
            return true;
          }
        }
      };

      self.process_chat(handler).await?;
      println!();
    }

    return Ok(String::new());
  }
}

fn print_indexed_list(lines: &[String]) {
  for (index, line) in lines.iter().enumerate() {
    println!("[{index}] {line}");
  }
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
