use anyhow::Error;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

pub use fenrir_core::client::api::{ApiResponse, ApiStatus};

pub fn success<T>(data: T, message: impl Into<String>) -> Json<ApiResponse<T>> {
  return Json(ApiResponse::success(data, message));
}

#[derive(Debug)]
pub struct ApiAccepted<T> {
  pub data: Option<T>,
  pub message: Option<String>,
}

impl<T> ApiAccepted<T> {
  pub fn accepted(message: impl Into<String>) -> Self {
    return Self {
      data: None,
      message: Some(message.into()),
    };
  }
}

impl<T: Serialize> IntoResponse for ApiAccepted<T> {
  fn into_response(self) -> Response {
    let body = ApiResponse {
      status: ApiStatus::Success,
      data: self.data,
      message: self.message,
    };
    return (StatusCode::ACCEPTED, Json(body)).into_response();
  }
}

#[derive(Debug)]
pub struct ApiError {
  pub status: StatusCode,
  pub message: String,
}

pub type ApiResult<T> = Result<Json<ApiResponse<T>>, ApiError>;

pub type ApiAction<T> = Result<ApiAccepted<T>, ApiError>;

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
