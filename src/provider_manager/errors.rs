use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderManagerError {
  #[error("No active provider configured.")]
  NoActiveProvider,
}

pub type ProviderManagerResult<T> = Result<T, ProviderManagerError>;
