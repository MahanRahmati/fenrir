use jiff::{Timestamp, ZonedDateTime, tz::TimeZone};

pub fn get_current_datetime() -> String {
  let now = ZonedDateTime::now();
  let datetime = now.strftime("%Y-%m-%d %H:%M:%S");
  let day = now.strftime("%A");
  return format!("Current date: {datetime} ({day})\n");
}

pub fn format_timestamp(timestamp: i64) -> String {
  let Ok(timestamp) = Timestamp::from_second(timestamp) else {
    return String::new();
  };
  let datetime = timestamp.to_zoned_datetime(TimeZone::system());
  return datetime.strftime("%Y-%m-%d %H:%M").to_string();
}
