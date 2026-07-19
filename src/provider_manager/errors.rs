use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderManagerError {}

pub type ProviderManagerResult<T> = Result<T, ProviderManagerError>;
