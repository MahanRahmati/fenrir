use crate::skill_manager::{Skill, SkillRegistry};

const PUNCTUATION: &[char] = &[',', ';', '.', '?', '!', ')', ']', '}'];

pub enum PathKind {
  Image,
  Text,
  Pdf,
}

pub fn extract_paths(content: &str) -> Vec<String> {
  let mut paths: Vec<String> = Vec::new();
  for token in content.split_whitespace() {
    let path = match token.strip_prefix('@') {
      Some(path) => path.trim_end_matches(PUNCTUATION),
      None => continue,
    };
    if path.is_empty() {
      continue;
    }
    if !paths.iter().any(|p| p == path) {
      paths.push(path.to_string());
    }
  }
  return paths;
}

pub fn strip_mentions(content: &str) -> String {
  return content
    .split_whitespace()
    .filter(|token| !token.starts_with('@'))
    .collect::<Vec<_>>()
    .join(" ");
}

pub fn strip_attached_mentions(content: &str, attached: &[String]) -> String {
  return content
    .split_whitespace()
    .filter(|token| {
      let Some(path) = token.strip_prefix('@') else {
        return true;
      };
      let path = path.trim_end_matches(PUNCTUATION);
      return !attached.iter().any(|attached| attached == path);
    })
    .collect::<Vec<_>>()
    .join(" ");
}

pub fn split_skill_mentions<'a>(
  content: &str,
  skills: &'a SkillRegistry,
) -> (String, Vec<&'a Skill>) {
  let mut mentioned: Vec<&Skill> = Vec::new();
  let mut text = String::new();
  let mut last = 0;
  for (start, end) in token_spans(content) {
    let token = &content[start..end];
    let Some(name) = token.strip_prefix('@') else {
      continue;
    };
    let name = name.trim_end_matches(PUNCTUATION);
    let Some(skill) = skills.find(name) else {
      continue;
    };
    text.push_str(&content[last..start]);
    last = end;
    if !mentioned.iter().any(|existing| existing.name == skill.name) {
      mentioned.push(skill);
    }
  }
  text.push_str(&content[last..]);
  return (text, mentioned);
}

pub fn skill_context(mentioned: &[&Skill], text: &str) -> Option<String> {
  if mentioned.is_empty() {
    return None;
  }
  let mut context = String::new();
  for skill in mentioned {
    context.push_str(&format!(
      "Skill: {} ({})\nDirectory: {}\n\n{}\n\n",
      skill.name,
      skill.scope.name(),
      skill.directory.display(),
      skill.content
    ));
  }
  context.push_str("End of skill instructions.");
  let text = text.trim();
  if !text.is_empty() {
    context.push_str("\n\n");
    context.push_str(text);
  }
  return Some(context);
}

fn token_spans(content: &str) -> Vec<(usize, usize)> {
  let mut spans = Vec::new();
  let mut start = None;
  for (index, character) in content.char_indices() {
    if character.is_whitespace() {
      if let Some(start) = start.take() {
        spans.push((start, index));
      }
    } else if start.is_none() {
      start = Some(index);
    }
  }
  if let Some(start) = start {
    spans.push((start, content.len()));
  }
  return spans;
}

pub fn classify_path(path: &str) -> Option<PathKind> {
  let extension = match path.rsplit_once('.') {
    Some((_, extension)) if !extension.is_empty() => extension,
    _ => return None,
  };

  return match extension.to_ascii_lowercase().as_str() {
    "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => Some(PathKind::Image),
    "pdf" => Some(PathKind::Pdf),
    _ => Some(PathKind::Text),
  };
}

pub fn mime_type(path: &str) -> Option<&str> {
  let extension = path.rsplit_once('.')?.1;
  return match extension.to_ascii_lowercase().as_str() {
    "png" => Some("image/png"),
    "jpg" | "jpeg" => Some("image/jpeg"),
    "webp" => Some("image/webp"),
    "gif" => Some("image/gif"),
    "bmp" => Some("image/bmp"),
    _ => None,
  };
}
