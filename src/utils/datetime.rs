use chrono::{Datelike, Weekday};

pub fn get_current_datetime() -> String {
  let now = chrono::Local::now();
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
  format!("Current date: {} ({})\n", datetime, day)
}
