use std::io::Write;

use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::app::{App, AppEvent};
use crate::input::confirm_with;
use crate::input::errors::InputError;
use crate::input::read_from_stdin;
use crate::session_manager::session::Session;
use crate::slash_commands::{SlashCommand, parse_command};
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
        Err(InputError::EmptyInput) => continue,
        Err(e) => return Err(RuntimeError::Input(e.to_string())),
      };

      match parse_command(&input) {
        Ok(Some(SlashCommand::Exit)) => break,
        Ok(Some(SlashCommand::Help)) => {
          println!("Available commands:");
          for cmd in SlashCommand::all() {
            println!("  {}: {}", cmd.name(), cmd.description());
          }
          continue;
        }
        Ok(Some(SlashCommand::Sessions)) => {
          self.handle_sessions_command().await?;
          continue;
        }
        Ok(Some(SlashCommand::New)) => {
          self.session_manager.clear_current_session();
          println!("Started a new session.");
          continue;
        }
        Ok(None) => {}
        Err(e) => {
          println!("Error: {e}");
          continue;
        }
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

  async fn handle_sessions_command(&mut self) -> RuntimeResult<()> {
    let sessions = self
      .session_manager
      .list_sessions()
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;
    let current_session = self.session_manager.get_current_session();
    if sessions.is_empty() {
      println!("No sessions found.");
      return Ok(());
    }
    for (index, session) in sessions.iter().enumerate() {
      let marker = match current_session {
        Some(current) if current.id == session.id => "*",
        _ => " ",
      };
      println!(
        "{marker}[{index}] {}  {}",
        session.id,
        format_timestamp(session.last_interaction)
      );
    }
    println!("Enter session index to switch to (or empty to skip): ");
    let Some(session_id) = read_session_index(&sessions)? else {
      return Ok(());
    };
    self
      .session_manager
      .load_session_from_db(session_id)
      .await
      .map_err(|e| RuntimeError::Session(e.to_string()))?;
    println!("--- Session {session_id} ---");
    if let Some(session) = self.session_manager.get_current_session() {
      print_session_messages(session);
    }
    return Ok(());
  }
}

fn read_session_index(sessions: &[Session]) -> RuntimeResult<Option<i64>> {
  let input = match read_from_stdin() {
    Ok(input) => input,
    Err(InputError::EmptyInput) => return Ok(None),
    Err(e) => return Err(RuntimeError::Input(e.to_string())),
  };
  let index = match input.trim().parse::<usize>() {
    Ok(index) => index,
    Err(_) => {
      println!("Invalid session index: {input}");
      return Ok(None);
    }
  };
  let session_id = match sessions.get(index) {
    Some(session) => session.id,
    None => {
      println!("No session at index {index}.");
      return Ok(None);
    }
  };
  return Ok(Some(session_id));
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
