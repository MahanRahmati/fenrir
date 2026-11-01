use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::tool_manager::cache::ToolCache;
use crate::tool_manager::helpers::get_optional_param;
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};

pub const WEATHER_TOOL_NAME: &str = "weather";

const BASE_URL: &str = "https://wttr.in/";
const REQUEST_TIMEOUT_SECS: u64 = 15;
const DEFAULT_FORECAST_DAYS: usize = 1;
const MAX_FORECAST_DAYS: usize = 3;
const NOON_HOUR_INDEX: usize = 4;

pub struct WeatherTool {
  cache: ToolCache<String, String>,
}

impl WeatherTool {
  pub fn new() -> Self {
    return Self {
      cache: ToolCache::new(),
    };
  }
}

#[derive(Deserialize)]
struct WeatherResponse {
  current_condition: Vec<CurrentCondition>,
  nearest_area: Vec<NearestArea>,
  weather: Vec<DailyWeather>,
}

#[derive(Deserialize)]
struct CurrentCondition {
  #[serde(rename = "temp_C")]
  temp_c: String,
  #[serde(rename = "FeelsLikeC")]
  feels_like_c: String,
  humidity: String,
  #[serde(rename = "weatherDesc")]
  weather_desc: Vec<Description>,
  #[serde(rename = "windspeedKmph")]
  wind_speed_kmph: String,
  #[serde(rename = "winddir16Point")]
  wind_dir: String,
  #[serde(rename = "precipMM")]
  precip_mm: String,
  observation_time: String,
}

#[derive(Deserialize)]
struct NearestArea {
  #[serde(rename = "areaName")]
  area_name: Vec<Description>,
  region: Vec<Description>,
  country: Vec<Description>,
}

#[derive(Deserialize)]
struct DailyWeather {
  date: String,
  #[serde(rename = "mintempC")]
  min_temp_c: String,
  #[serde(rename = "maxtempC")]
  max_temp_c: String,
  hourly: Vec<HourlyWeather>,
}

#[derive(Deserialize)]
struct HourlyWeather {
  #[serde(rename = "weatherDesc")]
  weather_desc: Vec<Description>,
  #[serde(rename = "chanceofrain")]
  chance_of_rain: String,
}

#[derive(Deserialize)]
struct Description {
  value: String,
}

#[async_trait::async_trait]
impl Tool for WeatherTool {
  fn name(&self) -> &str {
    return WEATHER_TOOL_NAME;
  }

  fn description(&self) -> &str {
    return "Get the current weather and a short forecast for a location. Omit the location to use the approximate current location.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "location": {
          "type": "string",
          "description": "City or place name, for example 'Tokyo'. Omit to use the approximate current location."
        },
        "days": {
          "type": "integer",
          "description": "Number of forecast days to include, from 1 to 3. Defaults to 1."
        }
      },
      "required": [],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  fn reset_cache(&self) {
    self.cache.clear();
  }

  async fn execute(
    &self,
    params: ToolParams,
    _context: &mut ToolContext<'_>,
  ) -> Result<String> {
    let location =
      get_optional_param::<String>(&params, WEATHER_TOOL_NAME, "location")
        .unwrap_or_default();
    let days = get_optional_param::<usize>(&params, WEATHER_TOOL_NAME, "days")
      .unwrap_or(DEFAULT_FORECAST_DAYS)
      .clamp(1, MAX_FORECAST_DAYS);

    let key = cache_key(&location);
    let response = match self.cache.get(&key) {
      Some(body) => serde_json::from_str::<WeatherResponse>(&body)
        .context("Failed to decode the cached weather response")?,
      None => {
        let body = fetch_weather(&location).await?;
        let response = serde_json::from_str::<WeatherResponse>(&body)
          .context("Failed to decode the weather response")?;
        self.cache.insert(key, body);
        response
      }
    };

    return Ok(format_weather(&response, days));
  }
}

async fn fetch_weather(location: &str) -> Result<String> {
  let url = build_url(location)?;
  let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
    .build()
    .context("Failed to create the weather HTTP client")?;
  let response = client
    .get(url)
    .header("User-Agent", "fenrir")
    .send()
    .await
    .context("Failed to fetch the weather")?;

  let status = response.status();
  if !status.is_success() {
    bail!("Weather service returned {status}");
  }

  return response
    .text()
    .await
    .context("Failed to read the weather response");
}

fn cache_key(location: &str) -> String {
  return location.trim().to_lowercase();
}

fn build_url(location: &str) -> Result<reqwest::Url> {
  let mut url =
    reqwest::Url::parse(BASE_URL).context("Failed to build the weather URL")?;
  let location = location.trim();
  if !location.is_empty() {
    url
      .path_segments_mut()
      .map_err(|_| anyhow!("Failed to build the weather URL"))?
      .push(location);
  }
  url.query_pairs_mut().append_pair("format", "j1");
  return Ok(url);
}

fn format_weather(response: &WeatherResponse, days: usize) -> String {
  let mut parts: Vec<String> = Vec::new();
  let area = response
    .nearest_area
    .first()
    .map(format_area)
    .unwrap_or_default();
  if area.is_empty() {
    parts.push("Weather:".to_string());
  } else {
    parts.push(format!("Weather for {area}:"));
  }

  if let Some(current) = response.current_condition.first() {
    let temp = &current.temp_c;
    let feels_like = &current.feels_like_c;
    parts.push(format!(
      "{}, {temp}°C (feels like {feels_like}°C)",
      description(&current.weather_desc),
    ));

    let humidity = &current.humidity;
    let wind_speed = &current.wind_speed_kmph;
    let wind_dir = &current.wind_dir;
    let precipitation = &current.precip_mm;
    parts.push(format!(
      "Humidity: {humidity}% | Wind: {wind_speed} km/h {wind_dir} | Precipitation: {precipitation} mm",
    ));

    let observed = &current.observation_time;
    parts.push(format!("Observed at {observed}"));
  }

  let forecast: Vec<String> = response
    .weather
    .iter()
    .take(days)
    .map(format_daily)
    .collect();
  if !forecast.is_empty() {
    let forecast = forecast.join("\n");
    parts.push(format!("Forecast:\n{forecast}"));
  }

  return parts.join("\n");
}

fn format_area(area: &NearestArea) -> String {
  let mut parts: Vec<String> = Vec::new();
  for values in [&area.area_name, &area.region, &area.country] {
    if let Some(value) = values.first() {
      let value = value.value.trim();
      if !value.is_empty() {
        parts.push(value.to_string());
      }
    }
  }
  return parts.join(", ");
}

fn format_daily(day: &DailyWeather) -> String {
  let hourly = day
    .hourly
    .get(NOON_HOUR_INDEX)
    .or_else(|| day.hourly.first());
  let conditions = hourly
    .map(|hourly| description(&hourly.weather_desc))
    .unwrap_or("Unknown");
  let rain = hourly
    .map(|hourly| hourly.chance_of_rain.as_str())
    .unwrap_or("unknown");
  let date = &day.date;
  let min_temp = &day.min_temp_c;
  let max_temp = &day.max_temp_c;
  return format!(
    "{date}: {conditions}, {min_temp}–{max_temp}°C, {rain}% chance of rain",
  );
}

fn description(values: &[Description]) -> &str {
  return values
    .first()
    .map(|value| value.value.trim())
    .filter(|value| !value.is_empty())
    .unwrap_or("Unknown");
}
