use serde::Serialize;

use crate::api_response::ApiResponse;

#[derive(Debug, Serialize)]
pub struct HealthData {
  name: &'static str,
  version: &'static str,
}

pub async fn health() -> ApiResponse<HealthData> {
  return ApiResponse::success(
    HealthData {
      name: "fenrir-server",
      version: env!("CARGO_PKG_VERSION"),
    },
    "Fenrir server is running",
  );
}
