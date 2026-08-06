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

pub async fn write_file(file_path: &str, content: &str) -> FileResult<()> {
  return tokio::fs::write(file_path, content)
    .await
    .map_err(|e| FileError::FileWrite(e.to_string()));
}

pub async fn copy_file(from_path: &str, to_path: &str) -> FileResult<()> {
  return tokio::fs::copy(from_path, to_path)
    .await
    .map_err(|e| FileError::FileCopy(e.to_string()))
    .map(|_| ());
}

pub async fn get_modified_time(
  file_path: &str,
) -> FileResult<std::time::SystemTime> {
  return tokio::fs::metadata(file_path)
    .await
    .map_err(|e| FileError::FileRead(e.to_string()))?
    .modified()
    .map_err(|e| FileError::FileRead(e.to_string()));
}

pub async fn get_file_size(file_path: &str) -> FileResult<u64> {
  return tokio::fs::metadata(file_path)
    .await
    .map_err(|e| FileError::FileRead(e.to_string()))
    .map(|meta| meta.len());
}

pub async fn read_dir(dir_path: &str) -> FileResult<Vec<(String, bool)>> {
  let mut entries = tokio::fs::read_dir(dir_path)
    .await
    .map_err(|e| FileError::DirectoryRead(format!("{dir_path}: {e}")))?;

  let mut results: Vec<(String, bool)> = Vec::new();

  while let Some(entry) = entries
    .next_entry()
    .await
    .map_err(|e| FileError::DirectoryRead(format!("{dir_path}: {e}")))?
  {
    let file_type = entry
      .file_type()
      .await
      .map_err(|e| FileError::DirectoryRead(format!("{dir_path}: {e}")))?;
    let name = entry.file_name().to_string_lossy().to_string();
    results.push((name, file_type.is_dir()));
  }

  results.sort_by(|a, b| a.0.cmp(&b.0));
  return Ok(results);
}
