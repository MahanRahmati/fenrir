use std::io::Write;

use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::app::{App, AppEvent};
use crate::input::confirm_with;
use crate::input::errors::InputError;
use crate::input::read_from_stdin;
use crate::slash_commands::{SlashCommand, parse_command};
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
        Ok(Some(cmd)) => println!("Unknown command: {}", cmd.name()),
        Ok(None) => {}
        Err(e) => {
          println!("Error: {}", e);
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
              print!("{}", text);
            } else {
              if reasoning_open {
                reasoning_open = false;
                if !reasoning_ends_with_newline {
                  println!();
                }
              }
              print!("{}", text);
            }
            stdout.flush().ok();
            return true;
          }
          AppEvent::Confirm(msg) => confirm_with(&format!("{} (y/N): ", msg)),
          AppEvent::ToolCall { name, arguments } => {
            if reasoning_open {
              reasoning_open = false;
              if !reasoning_ends_with_newline {
                println!();
              }
            }
            println!("[tool] {}({})", name, arguments);
            stdout.flush().ok();
            return true;
          }
          AppEvent::ToolResult { result, error } => {
            if error {
              println!("[error] {}", result);
            } else {
              println!("[result] {}", result);
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
