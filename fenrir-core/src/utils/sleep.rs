use std::future::Future;
use std::time::Duration;

use tokio::time::{self, Instant};

pub async fn delay(duration: Option<u64>) {
  if let Some(duration) = duration {
    time::sleep(Duration::from_secs(duration)).await;
  }
}

pub async fn milli_delay(duration: Option<u64>) {
  if let Some(duration) = duration {
    time::sleep(Duration::from_millis(duration)).await;
  }
}

pub async fn poll<T, F, Fut>(
  timeout: Duration,
  interval: Duration,
  mut condition: F,
) -> Option<T>
where
  F: FnMut() -> Fut,
  Fut: Future<Output = Option<T>>,
{
  let deadline = Instant::now() + timeout;
  loop {
    if let Some(value) = condition().await {
      return Some(value);
    }
    if Instant::now() >= deadline {
      return None;
    }
    time::sleep(interval).await;
  }
}
