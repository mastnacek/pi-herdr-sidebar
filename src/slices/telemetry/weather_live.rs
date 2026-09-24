//! Live weather slice — reads MET Norway (yr.no) Locationforecast 2.0 directly.
//!
//! Data source: `https://api.met.no/weatherapi/locationforecast/2.0/compact?lat=&lon=`
//! Terms of service: identification via `User-Agent` header, max 4 decimals in
//! coordinates, cached responses on disk (`Expires`/`If-Modified-Since` honored
//! by a simple TTL). No API key required.
//!
//! Eight preset locations cover the Czech Republic for trip planning; `w`
//! rotates them one step at a time. Default: Otovice u Broumova.
//!
//! Wire format, fetch, and parsing live in `yr_api.rs`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::yr_api::{fetch_locationforecast, parse_forecast};

/// How long a fetched forecast stays fresh (yr recommends honoring Expires; 30 min is safe).
const CACHE_TTL_SECS: u64 = 1800;

// ---------------------------------------------------------------------------
// Location presets
// ---------------------------------------------------------------------------

/// Preset locations covering the Czech Republic for trip planning:
/// N (Broumovsko, Krkonoše), W (Plzeňsko), center (Praha), S (Českobudějovicko),
/// SE (Brněnsko), E-SE (Zlínsko), E (Ostravsko).
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

pub const LOCATIONS: [WeatherLocation; 8] = [
    WeatherLocation {
        id: "otovice",
        name: "Otovice u Broumova",
        lat_x10k: 505936,
        lon_x10k: 163231,
    },
    WeatherLocation {
        id: "spindleruv",
        name: "Špindlerův Mlýn",
        lat_x10k: 507238,
        lon_x10k: 156244,
    },
    WeatherLocation {
        id: "praha",
        name: "Praha",
        lat_x10k: 500755,
        lon_x10k: 144378,
    },
    WeatherLocation {
        id: "plzen",
        name: "Plzeň",
        lat_x10k: 497384,
        lon_x10k: 133736,
    },
    WeatherLocation {
        id: "budejovice",
        name: "České Budějovice",
        lat_x10k: 489757,
        lon_x10k: 144749,
    },
    WeatherLocation {
        id: "brno",
        name: "Brno",
        lat_x10k: 491951,
        lon_x10k: 166068,
    },
    WeatherLocation {
        id: "zlin",
        name: "Zlín",
        lat_x10k: 492268,
        lon_x10k: 176687,
    },
    WeatherLocation {
        id: "ostrava",
        name: "Ostrava",
        lat_x10k: 498209,
        lon_x10k: 182609,
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
    /// Whether this frame came from the disk cache (offline fallback).
    pub from_cache: bool,
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

// ---------------------------------------------------------------------------
// Clipboard export (all locations × all days)
// ---------------------------------------------------------------------------

/// Local time label `DD.MM.YYYY HH:MM` (UTC+2, same convention as forecast grouping).
fn local_now_label() -> String {
    let secs = now_secs() as i64 + 2 * 3600;
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (y, m, d) = super::yr_api::civil_date(days);
    format!(
        "{:02}.{:02}.{} {:02}:{:02}",
        d,
        m,
        y,
        tod / 3600,
        (tod % 3600) / 60
    )
}

/// Plain-text forecast report for ALL preset locations × all days.
/// Uses fresh fetches (per-location cache holds only the current one), so the
/// first call blocks for a few seconds while curl runs sequentially.
pub fn collect_all_locations_report() -> String {
    let mut out = format!(
        "Předpověď počasí – ČR (vygenerováno {})\n",
        local_now_label()
    );

    for (i, loc) in LOCATIONS.iter().enumerate() {
        let t = refresh_weather(i, false);
        out.push_str(&format!("\n=== {} ===\n", loc.name));

        if let Some(err) = &t.error {
            out.push_str(&format!("chyba: {}\n", err));
            continue;
        }
        if let Some(cur) = &t.current {
            out.push_str(&format!(
                "aktuálně: {:.1}°C, {}, vítr {:.1} m/s\n",
                cur.temp_c, cur.symbol, cur.wind_ms
            ));
        }
        for day in &t.days {
            let precip = if day.precip_mm >= 0.2 {
                format!("  srážky {:.1} l/m²", day.precip_mm)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "{} {}  {} {:.0}°/{:.0}°{}\n",
                day.weekday, day.date, day.icon, day.temp_min, day.temp_max, precip
            ));
        }
    }
    out
}

/// Copy `text` to the system clipboard (Windows `clip`, macOS `pbcopy`, Linux `wl-copy`).
pub fn copy_to_clipboard(text: &str) -> bool {
    use std::io::Write;
    use std::process::Stdio;

    #[cfg(target_os = "windows")]
    {
        // clip.exe expects UTF-16LE; piping raw UTF-8 mangles Czech diacritics
        // (Předpověď → PÅ™edpovÄ›Ä). Convert first.
        let utf16: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let Ok(mut child) = Command::new("clip")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        if let Some(stdin) = child.stdin.as_mut() {
            if stdin.write_all(&utf16).is_err() {
                return false;
            }
        }
        drop(child.stdin.take());
        child.wait().map(|s| s.success()).unwrap_or(false)
    }

    #[cfg(target_os = "macos")]
    {
        let Ok(mut child) = Command::new("pbcopy").stdin(Stdio::piped()).spawn() else {
            return false;
        };
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
        }
        drop(child.stdin.take());
        child.wait().map(|s| s.success()).unwrap_or(false)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Wayland first, X11 fallback
        for bin in ["wl-copy", "xclip"] {
            let mut cmd = Command::new(bin);
            if bin == "xclip" {
                cmd.args(["-selection", "clipboard"]);
            }
            if let Ok(mut child) = cmd.stdin(Stdio::piped()).spawn() {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                drop(child.stdin.take());
                if child.wait().map(|s| s.success()).unwrap_or(false) {
                    return true;
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Main entry: telemetry for the selected location, honoring the disk cache.
/// Blocking network call (curl, 8 s budget) only when the cache is stale —
/// same pattern as the Antigravity quota slice.
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
