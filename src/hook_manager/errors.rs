use thiserror::Error;

#[derive(Error, Debug)]
pub enum HookManagerError {}

pub type HookManagerResult<T> = Result<T, HookManagerError>;
