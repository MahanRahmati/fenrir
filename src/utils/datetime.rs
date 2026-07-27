use chrono::{Datelike, Local, TimeZone, Weekday};

pub fn get_current_datetime() -> String {
  let now = Local::now();
  let datetime = now.format("%Y-%m-%d %H:%M:%S").to_string();
  let day = match now.weekday() {
    Weekday::Mon => "Monday",
    Weekday::Tue => "Tuesday",
    Weekday::Wed => "Wednesday",
    Weekday::Thu => "Thursday",
    Weekday::Fri => "Friday",
    Weekday::Sat => "Saturday",
    Weekday::Sun => "Sunday",
  };
  return format!("Current date: {} ({})\n", datetime, day);
}

pub fn format_timestamp(timestamp: i64) -> String {
  let dt = Local.timestamp_opt(timestamp, 0);
  return match dt {
    chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
    _ => "".to_string(),
  };
}
