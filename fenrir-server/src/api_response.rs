use anyhow::Error;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiStatus {
  Success,
  Error,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
  pub status: ApiStatus,
  pub data: Option<T>,
  pub message: Option<String>,
}

impl<T> ApiResponse<T> {
  pub fn success(data: T, message: impl Into<String>) -> Self {
    return Self {
      status: ApiStatus::Success,
      data: Some(data),
      message: Some(message.into()),
    };
  }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
  fn into_response(self) -> Response {
    return (StatusCode::OK, Json(self)).into_response();
  }
}

#[derive(Debug)]
pub struct ApiError {
  pub status: StatusCode,
  pub message: String,
}

pub type ApiResult<T> = Result<ApiResponse<T>, ApiError>;

pub type ApiStreamResult<T> = Result<T, ApiError>;

impl ApiError {
  pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
    return Self {
      status,
      message: message.into(),
    };
  }

  pub fn from_message(message: impl Into<String>) -> Self {
    let message = message.into();
    let status = if message == "Session not found" {
      StatusCode::NOT_FOUND
    } else if message.starts_with("Unknown persona")
      || message.starts_with("Unknown command")
      || message.starts_with("No session selected")
    {
      StatusCode::BAD_REQUEST
    } else if message.contains("busy") {
      StatusCode::CONFLICT
    } else {
      StatusCode::INTERNAL_SERVER_ERROR
    };
    return Self::new(status, message);
  }
}

impl From<Error> for ApiError {
  fn from(error: Error) -> Self {
    return Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
  }
}

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    let body = ApiResponse::<()> {
      status: ApiStatus::Error,
      data: None,
      message: Some(self.message),
    };
    return (self.status, Json(body)).into_response();
  }
}
