//! Local wall-clock reading: OS probe with a UTC fallback.

/// One calendar moment: year, month, day, hour, minute, second.
pub type CalendarMoment = (i64, u32, u32, u32, u32, u32);

/// Civil-date algorithm from Howard Hinnant over Unix seconds (UTC).
fn civil_from_unix(secs: u64) -> CalendarMoment {
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let hours = (day_secs / 3600) as u32;
    let minutes = ((day_secs % 3600) / 60) as u32;
    let seconds = (day_secs % 60) as u32;

    let z = (days as i64) + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32, hours, minutes, seconds)
}

#[cfg(windows)]
fn now_local() -> CalendarMoment {
    // GetLocalTime returns the OS-local wall time, DST-correct, no extra deps.
    #[repr(C)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    extern "system" {
        fn GetLocalTime(system_time: *mut SystemTime);
    }
    let mut st = SystemTime {
        year: 0,
        month: 0,
        day_of_week: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        milliseconds: 0,
    };
    unsafe {
        GetLocalTime(&mut st);
    }
    if st.year > 1601 && (1..=12).contains(&st.month) && (1..=31).contains(&st.day) {
        (
            st.year as i64,
            st.month as u32,
            st.day as u32,
            st.hour as u32,
            st.minute as u32,
            st.second as u32,
        )
    } else {
        local_fallback()
    }
}

#[cfg(unix)]
fn now_local() -> CalendarMoment {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let mut tm = libc::tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null_mut(),
    };
    let ok = unsafe { !libc::localtime_r(&secs, &mut tm).is_null() };
    if ok {
        (
            (tm.tm_year + 1900) as i64,
            (tm.tm_mon + 1) as u32,
            tm.tm_mday as u32,
            tm.tm_hour as u32,
            tm.tm_min as u32,
            tm.tm_sec as u32,
        )
    } else {
        local_fallback()
    }
}

/// UTC fallback for platforms where the local-time probe fails.
fn local_fallback() -> CalendarMoment {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    civil_from_unix(secs)
}

/// Current local time as a Czech inline stamp: `[10.2.2026 14:22:37]`.
///
/// Same shape `piprompt-core` stamps on a numbered entry: Czech date (no
/// leading zeros on day/month), zero-padded clock. The Scratchpad inserts it
/// right after a bare prefix, and [`crate::slices::spai_notes::note_writer`]
/// reads it back into the note's `timestamp` frontmatter, so a draft written
/// yesterday keeps yesterday's date when saved today.
pub fn current_stamp_czech() -> String {
    let (y, m, d, h, mi, s) = now_local();
    format!("[{}.{}.{} {:02}:{:02}:{:02}]", d, m, y, h, mi, s)
}

/// Current local date (`YYYY-MM-DD`) and timestamp (`YYYY-MM-DD HH:MM:SS`).
///
/// Uses the OS-local time (DST included) so notes created after midnight in
/// e.g. CEST do not get yesterday's filename date, as they would with plain
/// UTC arithmetic.
pub fn current_timestamp_and_date() -> (String, String) {
    let (y, m, d, h, mi, s) = now_local();
    let date_str = format!("{:04}-{:02}-{:02}", y, m, d);
    let time_str = format!("{} {:02}:{:02}:{:02}", date_str, h, mi, s);
    (date_str, time_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_uses_local_wall_time_format() {
        let (date, time) = current_timestamp_and_date();
        assert_eq!(date.len(), 10, "YYYY-MM-DD: {date}");
        assert!(time.starts_with(&date), "timestamp starts with the date: {time}");
        assert_eq!(time.len(), 19, "YYYY-MM-DD HH:MM:SS: {time}");
        // Sanity: local wall time is within 26 hours of UTC.
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(civil_from_unix(secs).0 >= 2020, "UTC sanity");
    }
}
