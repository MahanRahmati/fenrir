use std::path::Path;

use anyhow::{Context, Result, bail};

pub async fn remove_file(file_path: &str) -> Result<()> {
  let path = Path::new(file_path);
  return tokio::fs::remove_file(path)
    .await
    .context(format!("Cannot remove file '{file_path}'"));
}

pub async fn create_directory_all(dir_path: &str) -> Result<()> {
  return tokio::fs::create_dir_all(dir_path)
    .await
    .context(format!("Cannot create directory '{dir_path}'"));
}

pub async fn validate_file_exists(file_path: &str) -> Result<()> {
  if !file_exists(file_path).await {
    bail!("File '{file_path}' does not exist");
  }
  return Ok(());
}

pub async fn file_exists(file_path: &str) -> bool {
  return tokio::fs::metadata(file_path).await.is_ok();
}

pub async fn is_dir(file_path: &str) -> bool {
  return tokio::fs::metadata(file_path)
    .await
    .map(|metadata| metadata.is_dir())
    .unwrap_or(false);
}

pub async fn read_to_string(file_path: &str) -> Result<String> {
  return tokio::fs::read_to_string(file_path)
    .await
    .context(format!("Cannot read file '{file_path}'"));
}

pub async fn read_bytes(file_path: &str) -> Result<Vec<u8>> {
  return tokio::fs::read(file_path)
    .await
    .context(format!("Cannot read file '{file_path}'"));
}

pub async fn write_file(file_path: &str, content: &str) -> Result<()> {
  return tokio::fs::write(file_path, content)
    .await
    .context(format!("Cannot write to file '{file_path}'"));
}

pub async fn copy_file(from_path: &str, to_path: &str) -> Result<()> {
  return tokio::fs::copy(from_path, to_path)
    .await
    .context(format!("Cannot copy file '{from_path}' to '{to_path}'"))
    .map(|_| ());
}

pub async fn rename_file(from_path: &str, to_path: &str) -> Result<()> {
  return tokio::fs::rename(from_path, to_path)
    .await
    .context(format!("Cannot rename file '{from_path}' to '{to_path}'"));
}

pub async fn get_modified_time(
  file_path: &str,
) -> Result<std::time::SystemTime> {
  return tokio::fs::metadata(file_path)
    .await
    .context(format!("Cannot read file '{file_path}'"))?
    .modified()
    .context(format!("Cannot read modification time for '{file_path}'"));
}

pub async fn get_file_size(file_path: &str) -> Result<u64> {
  return tokio::fs::metadata(file_path)
    .await
    .context(format!("Cannot read file '{file_path}'"))
    .map(|meta| meta.len());
}

pub async fn read_dir(dir_path: &str) -> Result<Vec<(String, bool)>> {
  let mut entries = tokio::fs::read_dir(dir_path)
    .await
    .context(format!("Cannot read directory '{dir_path}'"))?;

  let mut results: Vec<(String, bool)> = Vec::new();

  while let Some(entry) = entries
    .next_entry()
    .await
    .context(format!("Cannot read directory '{dir_path}'"))?
  {
    let file_type = entry
      .file_type()
      .await
      .context(format!("Cannot read directory '{dir_path}'"))?;
    let name = entry.file_name().to_string_lossy().to_string();
    results.push((name, file_type.is_dir()));
  }

  results.sort_by(|a, b| a.0.cmp(&b.0));
  return Ok(results);
}
