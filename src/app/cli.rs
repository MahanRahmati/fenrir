use crate::app::App;
use crate::app::errors::{RuntimeError, RuntimeResult};
use crate::input::errors::InputError;
use crate::input::read_from_stdin;
use crate::slash_commands::{SlashCommand, parse_command};

impl App {
  pub async fn run(&mut self) -> RuntimeResult<String> {
    loop {
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

      print!("{}", self.process_chat().await?);
      println!();
    }

    return Ok(String::new());
  }
}
