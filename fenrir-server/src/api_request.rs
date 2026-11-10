use crate::api_response::ApiError;

pub trait ApiRequest {
  fn validate(&self) -> Result<(), ApiError>;
}
