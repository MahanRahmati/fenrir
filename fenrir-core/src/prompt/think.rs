const OPEN: &str = concat!("<", "think");
const CLOSE: &str = concat!("</", "think", ">");
const TICK: char = '\u{60}';

pub fn strip_inline_reasoning(content: &str) -> String {
  if let Some(index) = find_outside_code(content, CLOSE) {
    return content[index + CLOSE.len()..].trim_start().to_string();
  }
  if let Some(index) = find_outside_code(content, OPEN) {
    return content[..index].trim_end().to_string();
  }
  return content.to_string();
}

fn find_outside_code(content: &str, needle: &str) -> Option<usize> {
  let mut fenced = false;
  let mut inline = false;
  let mut line_start = true;
  let mut index = 0;
  while index < content.len() {
    let character = content[index..].chars().next()?;
    if line_start && is_fence_start(content, index) {
      fenced = !fenced;
      index += 3;
      line_start = false;
      continue;
    }
    if !fenced && character == TICK {
      inline = !inline;
    }
    if !fenced && !inline && content[index..].starts_with(needle) {
      return Some(index);
    }
    line_start = character == '\n';
    index += character.len_utf8();
  }
  return None;
}

fn is_fence_start(content: &str, index: usize) -> bool {
  let mut characters = content[index..].chars();
  return characters.next() == Some(TICK)
    && characters.next() == Some(TICK)
    && characters.next() == Some(TICK);
}
