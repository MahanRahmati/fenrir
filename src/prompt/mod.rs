use chrono::{Datelike, Utc, Weekday};

const SYSTEM_PROMPT: &str = "You are Fenrir, a helpful AI assistant running locally. Use tools when appropriate. Be concise and direct.";

pub struct Prompt {
  system_prompt: String,
}

impl Prompt {
  pub fn new() -> Self {
    let date_time_prompt = get_date_time_prompt();
    let full_prompt = format!("{}\n\n{}", SYSTEM_PROMPT, date_time_prompt,);
    return Self {
      system_prompt: full_prompt,
    };
  }

  pub fn get_system_prompt(&self) -> &str {
    return &self.system_prompt;
  }
}

fn get_date_time_prompt() -> String {
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
  return date_time_prompt;
}
