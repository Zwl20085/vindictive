//! Current weather from Open-Meteo (no API key). The place name is resolved
//! through their geocoding endpoint first, then the forecast endpoint is
//! queried for the current conditions and today's range.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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
const CONNECT_TIMEOUT_SECS: u64 = 5;
/// Longest wait between attempts while the weather service is unreachable.
pub const MAX_BACKOFF: Duration = Duration::from_secs(30 * 60);
pub const MAX_LOCATION_CHARS: usize = 80;

#[derive(Debug, thiserror::Error)]
pub enum WeatherError {
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("weather unavailable ({0})")]
    Offline(String),
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

/// The one HTTP client of the app (weather and paper metadata), so the
/// connection pool and TLS setup are shared instead of rebuilt per request.
pub(crate) fn http() -> reqwest::Result<&'static reqwest::Client> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()?;
    // A racing first call may have set it already; either client is fine.
    Ok(CLIENT.get_or_init(|| client))
}

/// Geocoded places by normalised query; place names do not move.
static PLACES: Mutex<BTreeMap<String, Place>> = Mutex::new(BTreeMap::new());
/// Per location: the last good reading and the current failure streak.
static HEALTH: Mutex<BTreeMap<String, Health>> = Mutex::new(BTreeMap::new());
/// Bound on both maps; the user has one location, this only stops growth
/// while they type a few different ones.
const MAX_CACHED_PLACES: usize = 16;

#[derive(Debug, Clone, Default)]
struct Health {
    last_good: Option<Weather>,
    failures: u32,
    last_error_at: Option<Instant>,
    last_error: String,
}

/// Wait before the next attempt after `failures` failures in a row:
/// nothing, then 1, 2, 4, ... minutes, capped at `MAX_BACKOFF`.
pub fn backoff(failures: u32) -> Duration {
    if failures == 0 {
        return Duration::ZERO;
    }
    let minutes = 1u64 << (failures - 1).min(16);
    Duration::from_secs(minutes * 60).min(MAX_BACKOFF)
}

/// True if the network may be tried again `since_error` after the last
/// failure of a streak of `failures`.
pub fn may_retry(failures: u32, since_error: Duration) -> bool {
    since_error >= backoff(failures)
}

fn cache_key(location: &str) -> String {
    location
        .trim()
        .chars()
        .take(MAX_LOCATION_CHARS)
        .collect::<String>()
        .to_lowercase()
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn remember<V>(map: &mut BTreeMap<String, V>, key: String, value: V) {
    if map.len() >= MAX_CACHED_PLACES && !map.contains_key(&key) {
        map.clear();
    }
    map.insert(key, value);
}

/// Resolve a free-text place name to coordinates, from memory when it was
/// resolved before.
pub async fn geocode(query: &str) -> Result<Place, WeatherError> {
    let key = cache_key(query);
    if let Some(place) = lock(&PLACES).get(&key).cloned() {
        return Ok(place);
    }
    let place = geocode_uncached(query).await?;
    remember(&mut lock(&PLACES), key, place.clone());
    Ok(place)
}

async fn geocode_uncached(query: &str) -> Result<Place, WeatherError> {
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
///
/// After a failure the network is left alone for a growing while (see
/// `backoff`), so polling while offline costs nothing instead of a timeout
/// each time. During that while, and on any failure, the last good reading
/// for this location is returned (with its own `fetched_at`) if there is one.
pub async fn fetch(location: &str, fetched_at: NaiveDateTime) -> Result<Weather, WeatherError> {
    let key = cache_key(location);
    let waiting = lock(&HEALTH).get(&key).and_then(|h| {
        let since = h.last_error_at?.elapsed();
        (!may_retry(h.failures, since)).then(|| (h.clone(), backoff(h.failures) - since))
    });
    if let Some((health, left)) = waiting {
        return health.last_good.ok_or_else(|| {
            WeatherError::Offline(format!(
                "{}; retrying in {} s",
                health.last_error,
                left.as_secs()
            ))
        });
    }
    let result = match geocode(location).await {
        Ok(place) => current(&place, fetched_at).await,
        Err(e) => Err(e),
    };
    let mut all = lock(&HEALTH);
    let health = all.get(&key).cloned().unwrap_or_default();
    match result {
        Ok(weather) => {
            let healthy = Health {
                last_good: Some(weather.clone()),
                ..Health::default()
            };
            remember(&mut all, key, healthy);
            Ok(weather)
        }
        Err(e) => {
            let failures = health.failures.saturating_add(1);
            log::warn!(
                "weather for {location:?} failed ({e}); next try in {} s",
                backoff(failures).as_secs()
            );
            let last_good = health.last_good.clone();
            let failed = Health {
                last_good: health.last_good,
                failures,
                last_error_at: Some(Instant::now()),
                last_error: e.to_string(),
            };
            remember(&mut all, key, failed);
            last_good.ok_or(e)
        }
    }
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
    fn backoff_doubles_from_one_minute_up_to_thirty() {
        let minutes: Vec<u64> = (0..=8).map(|n| backoff(n).as_secs() / 60).collect();
        assert_eq!(minutes, vec![0, 1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(backoff(u32::MAX), MAX_BACKOFF);
        assert!(may_retry(0, Duration::ZERO));
        assert!(!may_retry(1, Duration::from_secs(59)));
        assert!(may_retry(1, Duration::from_secs(60)));
        assert!(!may_retry(6, Duration::from_secs(29 * 60)));
        assert!(may_retry(40, MAX_BACKOFF));
    }

    #[test]
    fn cache_keys_ignore_case_and_padding() {
        assert_eq!(cache_key("  Tokyo "), cache_key("tokyo"));
        assert_ne!(cache_key("Tokyo"), cache_key("Kyoto"));
        let mut map = BTreeMap::new();
        for i in 0..MAX_CACHED_PLACES {
            remember(&mut map, i.to_string(), i);
        }
        remember(&mut map, "0".into(), 0);
        assert_eq!(
            map.len(),
            MAX_CACHED_PLACES,
            "updating a key keeps the rest"
        );
        remember(&mut map, "new".into(), 99);
        assert_eq!(map.len(), 1, "a full map starts over");
    }

    #[test]
    fn shared_client_is_built_once() {
        let a = http().unwrap() as *const reqwest::Client;
        let b = http().unwrap() as *const reqwest::Client;
        assert_eq!(a, b);
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
