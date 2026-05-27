use std::collections::HashMap;

use chrono::{Datelike, Utc, Weekday};

use crate::tool_manager::tool::Tool;

pub struct TimeTool;

impl Tool for TimeTool {
  fn name(&self) -> &str {
    return "time";
  }

  fn description(&self) -> &str {
    return "Returns the current date and time in UTC.";
  }

  fn execute(
    &self,
    _params: HashMap<String, String>,
  ) -> Result<String, String> {
    let now = Utc::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let datetime_str = now.format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let day_name = match now.weekday() {
      Weekday::Mon => "Monday",
      Weekday::Tue => "Tuesday",
      Weekday::Wed => "Wednesday",
      Weekday::Thu => "Thursday",
      Weekday::Fri => "Friday",
      Weekday::Sat => "Saturday",
      Weekday::Sun => "Sunday",
    };
    let date_time_prompt = format!(
      "Current date: {} ({})\nCurrent datetime: {}\n",
      date_str, day_name, datetime_str
    );
    return Ok(date_time_prompt);
  }
}
