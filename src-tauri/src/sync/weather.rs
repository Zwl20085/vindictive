//! Current weather from Open-Meteo (no API key). The place name is resolved
//! through their geocoding endpoint first, then the forecast endpoint is
//! queried for the current conditions and today's range.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

const GEOCODE_API: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST_API: &str = "https://api.open-meteo.com/v1/forecast";
const UA: &str = concat!(
    "vindictive/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Zwl20085/vindictive)"
);
const TIMEOUT_SECS: u64 = 15;
pub const MAX_LOCATION_CHARS: usize = 80;

#[derive(Debug, thiserror::Error)]
pub enum WeatherError {
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("no place called {0:?}")]
    NotFound(String),
    #[error("unexpected weather response: {0}")]
    Malformed(&'static str),
}

/// Mirrors `Weather` in `src/types.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    pub location: String,
    pub temperature_c: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high_c: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low_c: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub humidity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wind_kmh: Option<f64>,
    pub code: u16,
    pub is_day: bool,
    pub fetched_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}

fn http() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .build()
}

/// Resolve a free-text place name to coordinates.
pub async fn geocode(query: &str) -> Result<Place, WeatherError> {
    let query: String = query.trim().chars().take(MAX_LOCATION_CHARS).collect();
    if query.is_empty() {
        return Err(WeatherError::NotFound(query));
    }
    let json = http()?
        .get(GEOCODE_API)
        .query(&[("name", query.as_str()), ("count", "1"), ("format", "json")])
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    parse_geocode(&json).ok_or(WeatherError::NotFound(query))
}

/// Fetch current conditions for a place.
pub async fn current(place: &Place, fetched_at: NaiveDateTime) -> Result<Weather, WeatherError> {
    let lat = place.latitude.to_string();
    let lon = place.longitude.to_string();
    let json = http()?
        .get(FORECAST_API)
        .query(&[
            ("latitude", lat.as_str()),
            ("longitude", lon.as_str()),
            (
                "current",
                "temperature_2m,relative_humidity_2m,weather_code,wind_speed_10m,is_day",
            ),
            ("daily", "temperature_2m_max,temperature_2m_min"),
            ("forecast_days", "1"),
            ("timezone", "auto"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    parse_forecast(&json, &place.name, fetched_at)
}

/// Geocode then fetch, in one call.
pub async fn fetch(location: &str, fetched_at: NaiveDateTime) -> Result<Weather, WeatherError> {
    let place = geocode(location).await?;
    current(&place, fetched_at).await
}

/// First result of a geocoding response.
pub fn parse_geocode(json: &serde_json::Value) -> Option<Place> {
    let first = json.get("results")?.as_array()?.first()?;
    let name = first.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(Place {
        name: name.to_string(),
        latitude: first.get("latitude")?.as_f64()?,
        longitude: first.get("longitude")?.as_f64()?,
    })
}

/// Pull the fields we show out of a forecast response.
pub fn parse_forecast(
    json: &serde_json::Value,
    name: &str,
    fetched_at: NaiveDateTime,
) -> Result<Weather, WeatherError> {
    let current = json
        .get("current")
        .ok_or(WeatherError::Malformed("missing current"))?;
    let num = |key: &str| current.get(key).and_then(|v| v.as_f64());
    let temperature_c =
        num("temperature_2m").ok_or(WeatherError::Malformed("missing temperature"))?;
    let code = num("weather_code").unwrap_or(0.0) as u16;
    let is_day = num("is_day").map(|v| v >= 1.0).unwrap_or(true);
    let daily = |key: &str| json.get("daily")?.get(key)?.as_array()?.first()?.as_f64();
    Ok(Weather {
        location: name.to_string(),
        temperature_c,
        high_c: daily("temperature_2m_max"),
        low_c: daily("temperature_2m_min"),
        humidity: num("relative_humidity_2m"),
        wind_kmh: num("wind_speed_10m"),
        code,
        is_day,
        fetched_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 28)
            .unwrap()
            .and_hms_opt(14, 0, 0)
            .unwrap()
    }

    #[test]
    fn parses_geocode() {
        let json = serde_json::json!({
            "results": [{"name": "Tokyo", "latitude": 35.69, "longitude": 139.69, "country": "Japan"}]
        });
        let place = parse_geocode(&json).unwrap();
        assert_eq!(place.name, "Tokyo");
        assert!((place.latitude - 35.69).abs() < 1e-9);
        assert!(parse_geocode(&serde_json::json!({"results": []})).is_none());
        assert!(parse_geocode(&serde_json::json!({"generationtime_ms": 1.0})).is_none());
    }

    #[test]
    fn parses_forecast() {
        let json = serde_json::json!({
            "current": {
                "temperature_2m": 23.4,
                "relative_humidity_2m": 58,
                "weather_code": 2,
                "wind_speed_10m": 14.2,
                "is_day": 1
            },
            "daily": {
                "temperature_2m_max": [26.1],
                "temperature_2m_min": [15.7]
            }
        });
        let w = parse_forecast(&json, "Tokyo", at()).unwrap();
        assert_eq!(w.location, "Tokyo");
        assert_eq!(w.code, 2);
        assert!(w.is_day);
        assert_eq!(w.high_c, Some(26.1));
        assert_eq!(w.low_c, Some(15.7));
        assert_eq!(w.humidity, Some(58.0));
        assert_eq!(w.fetched_at, at());
    }

    #[test]
    fn forecast_without_current_is_an_error() {
        let json = serde_json::json!({"daily": {}});
        assert!(parse_forecast(&json, "x", at()).is_err());
        let json = serde_json::json!({"current": {"weather_code": 3}});
        assert!(parse_forecast(&json, "x", at()).is_err());
    }

    #[test]
    fn serialises_like_the_frontend_expects() {
        let w = Weather {
            location: "Tokyo".into(),
            temperature_c: 23.4,
            high_c: None,
            low_c: None,
            humidity: None,
            wind_kmh: None,
            code: 0,
            is_day: false,
            fetched_at: at(),
        };
        let text = serde_json::to_string(&w).unwrap();
        assert!(text.contains("\"fetched_at\":\"2026-09-28T14:00:00\""));
        assert!(!text.contains("high_c"));
        assert!(text.contains("\"is_day\":false"));
    }
}
