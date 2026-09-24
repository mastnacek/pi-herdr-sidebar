//! Live weather slice — reads MET Norway (yr.no) Locationforecast 2.0 directly.
//!
//! Data source: `https://api.met.no/weatherapi/locationforecast/2.0/compact?lat=&lon=`
//! Terms of service: identification via `User-Agent` header, max 4 decimals in
//! coordinates, cached responses on disk (`Expires`/`If-Modified-Since` honored
//! by a simple TTL). No API key required.
//!
//! Default location: Otovice u Broumova (CZ). Three more presets cover the rest
//! of the Czech Republic; `w` cycles them (click-to-rotate later).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// How long a fetched forecast stays fresh (yr recommends honoring Expires; 30 min is safe).
const CACHE_TTL_SECS: u64 = 1800;
/// Network timeout for the curl call.
const FETCH_TIMEOUT_SECS: u32 = 8;

/// Preset locations covering the Czech Republic (N/E/S/W quadrants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeatherLocation {
    pub id: &'static str,
    pub name: &'static str,
    /// Fixed-point lat/lon ×10000 (4 decimals max per yr.no ToS).
    pub lat_x10k: i32,
    pub lon_x10k: i32,
}

impl WeatherLocation {
    pub fn lat(&self) -> String {
        format!("{:.2}", self.lat_x10k as f64 / 10_000.0)
    }
    pub fn lon(&self) -> String {
        format!("{:.2}", self.lon_x10k as f64 / 10_000.0)
    }
}

pub const LOCATIONS: [WeatherLocation; 4] = [
    WeatherLocation {
        id: "otovice",
        name: "Otovice u Broumova",
        lat_x10k: 505936,
        lon_x10k: 163231,
    },
    WeatherLocation {
        id: "praha",
        name: "Praha",
        lat_x10k: 500755,
        lon_x10k: 144378,
    },
    WeatherLocation {
        id: "brno",
        name: "Brno",
        lat_x10k: 491951,
        lon_x10k: 166068,
    },
    WeatherLocation {
        id: "plzen",
        name: "Plzeň",
        lat_x10k: 497384,
        lon_x10k: 133736,
    },
];

pub fn location_by_index(idx: usize) -> WeatherLocation {
    LOCATIONS[idx % LOCATIONS.len()]
}

pub fn index_of_location(id: &str) -> usize {
    LOCATIONS.iter().position(|l| l.id == id).unwrap_or(0) // default: Otovice u Broumova
}

// ---------------------------------------------------------------------------
// Parsed forecast model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CurrentWeather {
    pub temp_c: f64,
    pub wind_ms: f64,
    pub wind_dir: u32,
    pub humidity: Option<f64>,
    pub pressure: Option<f64>,
    pub symbol: String,
    pub icon: &'static str,
    pub color: (u8, u8, u8),
}

#[derive(Debug, Clone)]
pub struct DayForecast {
    /// `2026-09-24` (local date)
    pub date: String,
    /// Czech weekday abbreviation (`Čt`, `Pá`, …)
    pub weekday: String,
    pub symbol: String,
    pub icon: &'static str,
    pub color: (u8, u8, u8),
    pub temp_min: f64,
    pub temp_max: f64,
    pub precip_mm: f64,
}

#[derive(Debug, Clone, Default)]
pub struct WeatherTelemetry {
    pub location_index: usize,
    pub location_name: String,
    pub updated_at: String,
    pub current: Option<CurrentWeather>,
    pub days: Vec<DayForecast>,
    pub error: Option<String>,
    /// Whether this frame came from disk cache (true) or a fresh fetch.
    pub from_cache: bool,
}

// ---------------------------------------------------------------------------
// yr.no JSON model (only the fields we consume)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct YrTimeSeries {
    time: String,
    data: YrData,
}

#[derive(Deserialize)]
struct YrData {
    instant: YrInstant,
    #[serde(rename = "next_1_hours", default)]
    next_1_hours: Option<YrPeriod>,
    #[serde(rename = "next_6_hours", default)]
    next_6_hours: Option<YrPeriod>,
    #[serde(rename = "next_12_hours", default)]
    next_12_hours: Option<YrPeriod>,
}

#[derive(Deserialize)]
struct YrInstant {
    details: YrInstantDetails,
}

#[derive(Deserialize)]
struct YrInstantDetails {
    #[serde(rename = "air_temperature", default)]
    air_temperature: Option<f64>,
    #[serde(rename = "wind_speed", default)]
    wind_speed: Option<f64>,
    #[serde(rename = "wind_from_direction", default)]
    wind_from_direction: Option<f64>,
    #[serde(rename = "relative_humidity", default)]
    relative_humidity: Option<f64>,
    #[serde(rename = "air_pressure_at_sea_level", default)]
    air_pressure_at_sea_level: Option<f64>,
}

#[derive(Deserialize)]
struct YrPeriod {
    summary: Option<YrSummary>,
    #[serde(default)]
    details: YrPrecipDetails,
}

#[derive(Deserialize)]
struct YrSummary {
    #[serde(rename = "symbol_code", default)]
    symbol_code: Option<String>,
}

#[derive(Deserialize, Default)]
struct YrPrecipDetails {
    #[serde(rename = "precipitation_amount", default)]
    precipitation_amount: Option<f64>,
}

#[derive(Deserialize)]
struct YrProperties {
    #[serde(rename = "updated_at", default)]
    updated_at: Option<String>,
    timeseries: Vec<YrTimeSeries>,
}

#[derive(Deserialize)]
struct YrRoot {
    properties: YrProperties,
}

// ---------------------------------------------------------------------------
// Symbol → icon & TrueColor mapping
// ---------------------------------------------------------------------------

/// Map a yr.no symbol_code (`lightrainshowers_day`) to a compact emoji icon
/// and a TrueColor RGB triple. Order matters: most specific first.
pub fn symbol_to_icon(symbol: &str) -> (&'static str, (u8, u8, u8)) {
    let s = symbol.to_lowercase();

    // Night variants get dimmer / moon-flavored icons
    let night = s.ends_with("_night") || s.contains("night");

    if s.contains("thunder") {
        ("⛈", (152, 122, 251)) // violet
    } else if s.contains("snow") {
        ("❄", (220, 240, 255)) // icy white
    } else if s.contains("sleet") {
        ("🌧", (4, 209, 249)) // cyan mix
    } else if s.contains("rain") {
        if s.contains("heavy") {
            ("🌧", (64, 130, 240)) // deep blue
        } else if s.contains("light") {
            ("🌦", (120, 180, 240)) // light blue
        } else {
            ("🌧", (80, 160, 240)) // blue
        }
    } else if s.contains("fog") {
        ("🌫", (135, 145, 170)) // slate
    } else if s.contains("partlycloudy") {
        if night {
            ("☁", (120, 124, 140))
        } else {
            ("⛅", (241, 252, 121)) // electric yellow mix
        }
    } else if s.contains("cloudy") {
        ("☁", (160, 168, 190)) // gray
    } else if s.contains("fair") {
        if night {
            ("🌙", (170, 160, 220))
        } else {
            ("🌤", (241, 252, 121))
        }
    } else if s.contains("clearsky") {
        if night {
            ("🌙", (170, 160, 220)) // lavender moon
        } else {
            ("☀", (250, 200, 60)) // sun yellow
        }
    } else {
        ("•", (135, 145, 170))
    }
}

/// Czech weekday abbreviation from day offset since epoch (UTC days).
fn weekday_cs(days_since_epoch: i64) -> &'static str {
    // 1970-01-01 was a Thursday (index 4 with Monday=0)
    const NAMES: [&str; 7] = ["Po", "Út", "St", "Čt", "Pá", "So", "Ne"];
    let idx = ((days_since_epoch + 3).rem_euclid(7)) as usize;
    NAMES[idx]
}

/// Date `YYYY-MM-DD` from days since epoch (Howard Hinnant's civil_from_days).
fn civil_date(days: i64) -> (i64, u64, u64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u64;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u64;
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

// ---------------------------------------------------------------------------
// Location selection persistence + disk cache
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct WeatherCacheFile {
    /// Selected location id (default `otovice`).
    location: Option<String>,
    /// Epoch seconds of the last successful fetch for `location`.
    fetched_at: u64,
    /// Raw yr.no JSON body.
    body: String,
}

fn cache_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("HERDR_PLUGIN_STATE_DIR") {
        return Some(PathBuf::from(dir).join("weather-cache.json"));
    }
    let home = crate::shared::dirs_home()?;
    Some(
        home.join(".pi")
            .join("agent")
            .join("pi-sidebar")
            .join("weather-cache.json"),
    )
}

fn load_cache() -> Option<WeatherCacheFile> {
    let path = cache_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_cache(cache: &WeatherCacheFile) {
    let Some(path) = cache_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, json);
    }
}

/// Load selected location index (default 0 = Otovice u Broumova).
pub fn load_selected_location() -> usize {
    load_cache()
        .and_then(|c| c.location)
        .map(|id| index_of_location(&id))
        .unwrap_or(0)
}

/// Persist selected location id.
pub fn save_selected_location(index: usize) {
    let mut cache = load_cache().unwrap_or_default();
    cache.location = Some(location_by_index(index).id.to_string());
    save_cache(&cache);
}

/// Main entry: telemetry for the selected location, honoring the disk cache.
/// Blocking network call (curl, `FETCH_TIMEOUT_SECS` budget) only when the
/// cache is stale — same pattern as the Antigravity quota slice.
pub fn refresh_weather(location_index: usize, force: bool) -> WeatherTelemetry {
    let loc = location_by_index(location_index);

    let cache = load_cache().unwrap_or_default();
    let fresh = now_secs()
        .checked_sub(cache.fetched_at)
        .map(|age| age < CACHE_TTL_SECS)
        .unwrap_or(false);

    let same_loc = cache
        .location
        .as_deref()
        .map(|id| id == loc.id)
        .unwrap_or(false);

    let body: Option<(String, bool)> = if !force && fresh && same_loc && !cache.body.is_empty() {
        Some((cache.body.clone(), true))
    } else {
        fetch_locationforecast(&loc).map(|b| (b, false))
    };

    let Some((body, from_cache)) = body else {
        // Network failed: fall back to any cached body for this location, else error.
        if same_loc && !cache.body.is_empty() {
            return parse_forecast(&cache.body, location_index, true, None);
        }
        return WeatherTelemetry {
            location_index,
            location_name: loc.name.to_string(),
            error: Some("yr.no nedostupné".to_string()),
            ..Default::default()
        };
    };

    // Persist cache on fresh fetch
    if !from_cache {
        save_cache(&WeatherCacheFile {
            location: Some(loc.id.to_string()),
            fetched_at: now_secs(),
            body: body.clone(),
        });
    }

    parse_forecast(&body, location_index, from_cache, None)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Blocking curl fetch of Locationforecast 2.0 compact for a location.
fn fetch_locationforecast(loc: &WeatherLocation) -> Option<String> {
    let url = format!(
        "https://api.met.no/weatherapi/locationforecast/2.0/compact?lat={}&lon={}",
        loc.lat(),
        loc.lon()
    );
    let output = Command::new("curl")
        .args([
            "-s",
            "-m",
            &FETCH_TIMEOUT_SECS.to_string(),
            "-A",
            "herdr-pi-sidebar/0.1 github.com/mastnacek/pi-herdr-sidebar",
            &url,
        ])
        .output()
        .ok()?;

    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Parse the yr.no compact payload into current conditions + 7-day forecast.
fn parse_forecast(
    body: &str,
    location_index: usize,
    from_cache: bool,
    error: Option<String>,
) -> WeatherTelemetry {
    let loc = location_by_index(location_index);
    let mut t = WeatherTelemetry {
        location_index,
        location_name: loc.name.to_string(),
        from_cache,
        error,
        ..Default::default()
    };

    let Ok(root) = serde_json::from_str::<YrRoot>(body) else {
        t.error = Some("nečitelná odpověď yr.no".to_string());
        return t;
    };
    t.updated_at = root.properties.updated_at.unwrap_or_default();

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    // Convert UTC epoch to local epoch (CEST = UTC+2; CST winter = UTC+1 —
    // a fixed +2 keeps summer correct, winter off by 1h which is irrelevant
    // for daily grouping near noon).
    const LOCAL_OFFSET_SECS: i64 = 2 * 3600;

    struct Entry {
        epoch: i64,
        temp: Option<f64>,
        wind: Option<f64>,
        wind_dir: Option<f64>,
        humidity: Option<f64>,
        pressure: Option<f64>,
        symbol_12h: Option<String>,
        precip_1h: Option<f64>,
        precip_6h: Option<f64>,
    }
    let mut entries: Vec<Entry> = Vec::new();

    for ts in &root.properties.timeseries {
        let epoch = crate::slices::telemetry::skills_live::iso_to_epoch_ms(&ts.time)
            .map(|ms| ms as i64)
            .unwrap_or(0);
        let d = &ts.data;
        entries.push(Entry {
            epoch,
            temp: d.instant.details.air_temperature,
            wind: d.instant.details.wind_speed,
            wind_dir: d.instant.details.wind_from_direction,
            humidity: d.instant.details.relative_humidity,
            pressure: d.instant.details.air_pressure_at_sea_level,
            symbol_12h: d
                .next_12_hours
                .as_ref()
                .and_then(|p| p.summary.as_ref())
                .and_then(|s| s.symbol_code.clone())
                .or_else(|| {
                    d.next_1_hours
                        .as_ref()
                        .and_then(|p| p.summary.as_ref())
                        .and_then(|s| s.symbol_code.clone())
                }),
            precip_1h: d
                .next_1_hours
                .as_ref()
                .and_then(|p| p.details.precipitation_amount),
            precip_6h: d
                .next_6_hours
                .as_ref()
                .and_then(|p| p.details.precipitation_amount),
        });
    }

    if entries.is_empty() {
        t.error = Some("prázdná předpověď".to_string());
        return t;
    }

    // ---- Current conditions: nearest entry not in the future ----
    let current_entry = entries
        .iter()
        .filter(|e| e.epoch <= now_ms / 1000)
        .max_by_key(|e| e.epoch)
        .or_else(|| entries.first());

    if let Some(e) = current_entry {
        if let Some(temp) = e.temp {
            let symbol = e
                .symbol_12h
                .clone()
                .unwrap_or_else(|| "clearsky_day".to_string());
            let (icon, color) = symbol_to_icon(&symbol);
            t.current = Some(CurrentWeather {
                temp_c: temp,
                wind_ms: e.wind.unwrap_or(0.0),
                wind_dir: e.wind_dir.map(|d| d as u32).unwrap_or(0),
                humidity: e.humidity,
                pressure: e.pressure,
                symbol,
                icon,
                color,
            });
        }
    }

    // ---- 7-day forecast: group by local date, today first ----
    // Local day index = floor((epoch + offset) / 86400).
    let mut days: Vec<(i64, Vec<&Entry>)> = Vec::new();
    for e in &entries {
        let local_day = (e.epoch + LOCAL_OFFSET_SECS).div_euclid(86_400);
        match days.last_mut() {
            Some((d, list)) if *d == local_day => list.push(e),
            _ => days.push((local_day, vec![e])),
        }
    }

    // Anchor: local "today" (day containing now).
    let today = (now_ms / 1000 + LOCAL_OFFSET_SECS).div_euclid(86_400);

    for (day, list) in days.iter().take(8) {
        if *day < today || t.days.len() >= 7 {
            continue;
        }
        let (y, m, d) = civil_date(*day);
        let temps: Vec<f64> = list.iter().filter_map(|e| e.temp).collect();
        if temps.is_empty() {
            continue;
        }

        // Day symbol: entry closest to 12:00 local with a 12h summary.
        let noon = day * 86_400 + 12 * 3600 - LOCAL_OFFSET_SECS;
        let symbol = list
            .iter()
            .min_by_key(|e| (e.epoch - noon).abs())
            .and_then(|e| e.symbol_12h.clone())
            .unwrap_or_else(|| "clearsky_day".to_string());
        let (icon, color) = symbol_to_icon(&symbol);

        let precip: f64 = list
            .iter()
            .map(|e| e.precip_1h.or(e.precip_6h).unwrap_or(0.0))
            .sum();

        t.days.push(DayForecast {
            date: format!("{:04}-{:02}-{:02}", y, m, d),
            weekday: weekday_cs(*day).to_string(),
            symbol,
            icon,
            color,
            temp_min: temps.iter().cloned().fold(f64::INFINITY, f64::min),
            temp_max: temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            precip_mm: precip,
        });
    }

    t
}
