use std::path::{Component, Path, PathBuf};

use crate::utils::xdg;

pub fn is_path_within(path_str: &str, cwd: &Path) -> bool {
  return resolve_path(path_str, cwd).starts_with(cwd);
}

pub fn resolve_path(path_str: &str, cwd: &Path) -> PathBuf {
  let expanded = expand_tilde(path_str);
  let path = Path::new(&expanded);
  let resolved = if path.is_relative() {
    cwd.join(path)
  } else {
    path.to_path_buf()
  };

  return match std::fs::canonicalize(&resolved) {
    Ok(p) => p,
    Err(_) => normalize(&resolved),
  };
}

fn expand_tilde(path_str: &str) -> PathBuf {
  let Some(home) = xdg::home_dir() else {
    return PathBuf::from(path_str);
  };

  if path_str == "~" {
    return home;
  }

  return match path_str.strip_prefix("~/") {
    Some(rest) => home.join(rest),
    None => PathBuf::from(path_str),
  };
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
