use std::path::PathBuf;

use xdg::BaseDirectories;

use crate::config::DEFAULT_DIRECTORY;

pub fn data_home() -> Result<PathBuf, String> {
  let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
  return xdg_dirs
    .get_data_home()
    .ok_or_else(|| "cannot determine the data home directory".to_string());
}

pub fn find_config_file(name: &str) -> Option<PathBuf> {
  let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
  return xdg_dirs.find_config_file(name);
}

pub fn place_config_file(name: &str) -> Result<PathBuf, String> {
  let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
  return xdg_dirs
    .place_config_file(name)
    .map_err(|e| format!("cannot place config file '{name}': {e}"));
}

pub fn place_data_file(name: &str) -> Result<PathBuf, String> {
  let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
  return xdg_dirs
    .place_data_file(name)
    .map_err(|e| format!("cannot place data file '{name}': {e}"));
}
