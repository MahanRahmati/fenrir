use jiff::Timestamp;

pub fn format_duration(seconds: u64) -> String {
  let hours = seconds / 3600;
  let minutes = (seconds % 3600) / 60;
  let remaining = seconds % 60;
  let mut parts = Vec::new();
  if hours > 0 {
    parts.push(format!("{hours} {}", unit(hours, "hour")));
  }
  if minutes > 0 {
    parts.push(format!("{minutes} {}", unit(minutes, "minute")));
  }
  if remaining > 0 || parts.is_empty() {
    parts.push(format!("{remaining} {}", unit(remaining, "second")));
  }
  return parts.join(" ");
}

pub fn working_for(seconds: u64) -> String {
  return format!("Working for {}", format_duration(seconds));
}

pub fn finished_in(seconds: u64) -> String {
  return format!("Finished in {}", format_duration(seconds));
}

pub fn elapsed_seconds(started_at: i64) -> u64 {
  let now = Timestamp::now().as_second();
  return now.saturating_sub(started_at).max(0) as u64;
}

fn unit(value: u64, name: &str) -> String {
  if value == 1 {
    return name.to_string();
  }
  return format!("{name}s");
}
