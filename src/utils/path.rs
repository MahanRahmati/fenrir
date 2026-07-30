use std::path::{Component, Path, PathBuf};

pub fn is_path_within(path_str: &str, cwd: &Path) -> bool {
  let path = Path::new(path_str);
  let resolved = if path.is_relative() {
    cwd.join(path)
  } else {
    path.to_path_buf()
  };

  let normalized = match std::fs::canonicalize(&resolved) {
    Ok(p) => p,
    Err(_) => normalize(&resolved),
  };

  return normalized.starts_with(cwd);
}

fn normalize(path: &Path) -> PathBuf {
  let mut result = PathBuf::new();
  for component in path.components() {
    match component {
      Component::ParentDir => {
        result.pop();
      }
      Component::CurDir => {}
      component => result.push(component),
    }
  }
  return result;
}
