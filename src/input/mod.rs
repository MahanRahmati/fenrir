use anyhow::{Context, Result};

pub fn read_from_stdin() -> Result<Option<String>> {
  let _ = std::io::Write::flush(&mut std::io::stdout());
  let mut input = String::new();
  let bytes_read = std::io::stdin()
    .read_line(&mut input)
    .context("Cannot read input from stdin")?;
  if bytes_read == 0 {
    return Ok(None);
  }
  let input = input.trim().to_string();
  return Ok((!input.is_empty()).then_some(input));
}

fn confirm(prompt: &str) -> Result<bool> {
  print!("{prompt}");
  let Some(input) = read_from_stdin()? else {
    return Ok(false);
  };
  let answer = input.trim().to_lowercase();
  return Ok(answer == "y" || answer == "yes");
}

pub fn confirm_with(prompt: &str) -> bool {
  return confirm(prompt).unwrap_or_default();
}
