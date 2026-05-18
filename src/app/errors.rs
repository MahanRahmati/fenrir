use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuntimeError {}

pub type RuntimeResult<T> = Result<T, RuntimeError>;
