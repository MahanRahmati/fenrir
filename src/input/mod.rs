pub mod errors;

use crate::input::errors::{InputError, InputResult};

pub fn read_from_stdin() -> InputResult<String> {
  let _ = std::io::Write::flush(&mut std::io::stdout());
  let mut input = String::new();
  let bytes_read = std::io::stdin()
    .read_line(&mut input)
    .map_err(|e| InputError::StdinRead(e.to_string()))?;
  if bytes_read == 0 {
    return Err(InputError::Eof);
  }
  let input = input.trim().to_string();
  return match input.is_empty() {
    true => Err(InputError::EmptyInput),
    false => Ok(input),
  };
}

fn confirm(prompt: &str) -> InputResult<bool> {
  print!("{prompt}");
  let input = read_from_stdin()?;
  let answer = input.trim().to_lowercase();
  return Ok(answer == "y" || answer == "yes");
}

pub fn confirm_with(prompt: &str) -> bool {
  return confirm(prompt).unwrap_or_default();
}
