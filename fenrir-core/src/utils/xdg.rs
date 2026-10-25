use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use xdg::BaseDirectories;

use crate::config::DEFAULT_DIRECTORY;

pub fn data_home() -> Result<PathBuf> {
  let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
  let Some(data_home) = xdg_dirs.get_data_home() else {
    bail!("cannot determine the data home directory");
  };
  return Ok(data_home);
}

pub fn find_config_file(name: &str) -> Option<PathBuf> {
  let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
  return xdg_dirs.find_config_file(name);
}

pub fn place_config_file(name: &str) -> Result<PathBuf> {
  let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
  return xdg_dirs
    .place_config_file(name)
    .context(format!("Cannot place config file '{name}'"));
}

pub fn place_data_file(name: &str) -> Result<PathBuf> {
  let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
  return xdg_dirs
    .place_data_file(name)
    .context(format!("Cannot place data file '{name}'"));
}

pub fn place_state_file(name: &str) -> Result<PathBuf> {
  let xdg_dirs = BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
  return xdg_dirs
    .place_state_file(name)
    .context(format!("Cannot place state file '{name}'"));
}
