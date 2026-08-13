use thiserror::Error;

#[derive(Error, Debug)]
pub enum FileError {
  #[error("Cannot create directory '{0}'. Please check permissions.")]
  DirectoryCreate(String),

  #[error(
    "Cannot remove file '{0}'. Please check if the file exists and you have permission to delete it."
  )]
  FileRemove(String),

  #[error(
    "Cannot read file '{0}'. Please check if the file exists and you have permission to access it."
  )]
  FileRead(String),

  #[error("File not found: '{0}'. Please verify the file path and try again.")]
  FileNotFound(String),

  #[error("Cannot write to file '{0}'. Please check permissions.")]
  FileWrite(String),

  #[error("Cannot copy file '{0}'. Please check permissions.")]
  FileCopy(String),

  #[error("Cannot rename file '{0}'. Please check permissions.")]
  FileRename(String),

  #[error("Cannot read directory '{0}'. Please check permissions.")]
  DirectoryRead(String),
}

pub type FileResult<T> = Result<T, FileError>;
