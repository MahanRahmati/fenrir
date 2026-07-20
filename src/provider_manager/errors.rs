use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderManagerError {
  #[error("No active provider configured.")]
  NoActiveProvider,

  #[error("No active model configured.")]
  NoActiveModel,

  #[error("Provider error: {0}")]
  ProviderError(String),

  #[error("Empty response choices.")]
  EmptyResponseChoices,
}

pub type ProviderManagerResult<T> = Result<T, ProviderManagerError>;
