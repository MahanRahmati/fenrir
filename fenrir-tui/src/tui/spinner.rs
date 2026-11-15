use std::time::{Duration, Instant};

const STATES: [&str; 4] = ["●∙∙∙", "∙●∙∙", "∙∙●∙", "∙∙∙●"];
const ICON_STATES: [&str; 4] = ["◐", "◓", "◑", "◒"];
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
    return STATES[self.tick(STATES.len())];
  }

  pub fn icon(&self) -> &str {
    return ICON_STATES[self.tick(ICON_STATES.len())];
  }

  fn tick(&self, states: usize) -> usize {
    let tick =
      self.started_at.elapsed().as_millis() / DURATION.as_millis().max(1);
    return (tick as usize) % states;
  }
}
