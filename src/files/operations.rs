use std::path::Path;

use crate::files::errors::{FileError, FileResult};

pub async fn remove_file(file_path: &str) -> FileResult<()> {
  let path = Path::new(file_path);
  return tokio::fs::remove_file(path)
    .await
    .map_err(|e| FileError::FileRemove(e.to_string()));
}

pub async fn create_directory_all(dir_path: &str) -> FileResult<()> {
  return tokio::fs::create_dir_all(dir_path)
    .await
    .map_err(|e| FileError::DirectoryCreate(e.to_string()));
}

pub async fn validate_file_exists(file_path: &str) -> FileResult<()> {
  if !file_exists(file_path).await {
    return Err(FileError::FileNotFound(file_path.to_string()));
  }
  return Ok(());
}

pub async fn file_exists(file_path: &str) -> bool {
  return tokio::fs::metadata(file_path).await.is_ok();
}

pub async fn read_to_string(file_path: &str) -> FileResult<String> {
  return tokio::fs::read_to_string(file_path)
    .await
    .map_err(|e| FileError::FileRead(e.to_string()));
}
