use std::time::Duration;

use tokio::time;

pub async fn delay(duration: Option<u64>) {
  if let Some(duration) = duration {
    time::sleep(Duration::from_secs(duration)).await;
  }
}
