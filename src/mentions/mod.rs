const PUNCTUATION: &[char] = &[',', ';', '.', '?', '!', ')', ']', '}'];

pub enum PathKind {
  Image,
  Text,
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

pub fn classify_path(path: &str) -> Option<PathKind> {
  let extension = match path.rsplit_once('.') {
    Some((_, extension)) if !extension.is_empty() => extension,
    _ => return None,
  };

  return match extension.to_ascii_lowercase().as_str() {
    "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => Some(PathKind::Image),
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
