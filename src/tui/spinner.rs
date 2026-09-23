use std::time::{Duration, Instant};

const STATES: [&'static str; 4] = ["●∙∙∙", "∙●∙∙", "∙∙●∙", "∙∙∙●"];
const DURATION: Duration = Duration::from_millis(200);

pub struct Spinner {
  started_at: Instant,
}

impl Spinner {
  pub fn new() -> Self {
    return Self {
      started_at: Instant::now(),
    };
  }

  pub fn frame(&self) -> &str {
    let tick =
      self.started_at.elapsed().as_millis() / DURATION.as_millis().max(1);
    let index = (tick as usize) % STATES.len();
    return STATES[index];
  }
}
