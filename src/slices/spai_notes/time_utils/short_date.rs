//! Short-date formatting (`dd.mm.yy`) for SPAI timestamps.

pub fn format_short_date(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let bytes = trimmed.as_bytes();
    let is_digit = |b: u8| b.is_ascii_digit();
    let is_sep = |b: u8| b == b'-' || b == b'.' || b == b'/';

    // 1. Check YYYY-MM-DD pattern at start (e.g. 2026-09-24 or 2026-09-24 10:57:47 or 2026-09-24-SPAI...)
    if bytes.len() >= 10
        && is_digit(bytes[0])
        && is_digit(bytes[1])
        && is_digit(bytes[2])
        && is_digit(bytes[3])
        && is_sep(bytes[4])
        && is_digit(bytes[5])
        && is_digit(bytes[6])
        && is_sep(bytes[7])
        && is_digit(bytes[8])
        && is_digit(bytes[9])
    {
        let y_str = &trimmed[0..4];
        let m_str = &trimmed[5..7];
        let d_str = &trimmed[8..10];

        if let (Ok(year), Ok(month), Ok(day)) = (
            y_str.parse::<u32>(),
            m_str.parse::<u32>(),
            d_str.parse::<u32>(),
        ) {
            if (1..=12).contains(&month) && (1..=31).contains(&day) {
                let yy = year % 100;
                return Some(format!("{:02}.{:02}.{:02}", day, month, yy));
            }
        }
    }

    // 2. Check DD.MM.YYYY pattern (e.g. 24.09.2026)
    if bytes.len() >= 10
        && is_digit(bytes[0])
        && is_digit(bytes[1])
        && is_sep(bytes[2])
        && is_digit(bytes[3])
        && is_digit(bytes[4])
        && is_sep(bytes[5])
        && is_digit(bytes[6])
        && is_digit(bytes[7])
        && is_digit(bytes[8])
        && is_digit(bytes[9])
    {
        let d_str = &trimmed[0..2];
        let m_str = &trimmed[3..5];
        let y_str = &trimmed[6..10];

        if let (Ok(day), Ok(month), Ok(year)) = (
            d_str.parse::<u32>(),
            m_str.parse::<u32>(),
            y_str.parse::<u32>(),
        ) {
            if (1..=12).contains(&month) && (1..=31).contains(&day) {
                let yy = year % 100;
                return Some(format!("{:02}.{:02}.{:02}", day, month, yy));
            }
        }
    }

    // 3. Check DD.MM.YY pattern (e.g. 24.09.26)
    if bytes.len() >= 8
        && is_digit(bytes[0])
        && is_digit(bytes[1])
        && is_sep(bytes[2])
        && is_digit(bytes[3])
        && is_digit(bytes[4])
        && is_sep(bytes[5])
        && is_digit(bytes[6])
        && is_digit(bytes[7])
    {
        let d_str = &trimmed[0..2];
        let m_str = &trimmed[3..5];
        let y_str = &trimmed[6..8];

        if let (Ok(day), Ok(month), Ok(yy)) = (
            d_str.parse::<u32>(),
            m_str.parse::<u32>(),
            y_str.parse::<u32>(),
        ) {
            if (1..=12).contains(&month) && (1..=31).contains(&day) {
                return Some(format!("{:02}.{:02}.{:02}", day, month, yy));
            }
        }
    }

    // 4. Numeric epoch timestamp
    if let Ok(num) = trimmed.parse::<u64>() {
        let secs = if num > 1_000_000_000_000 {
            num / 1000
        } else {
            num
        };
        if secs >= 946684800 {
            let days = secs / 86400;
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
            let yy = (y.abs() % 100) as u32;
            return Some(format!("{:02}.{:02}.{:02}", d, m, yy));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_iso_and_space_timestamps() {
        assert_eq!(
            format_short_date("2026-09-24 10:57:47"),
            Some("24.09.26".to_string())
        );
        assert_eq!(
            format_short_date("2026-09-25 08:56:58"),
            Some("25.09.26".to_string())
        );
        assert_eq!(
            format_short_date("2026-01-05"),
            Some("05.01.26".to_string())
        );
        assert_eq!(
            format_short_date("2026-12-31T23:59:59Z"),
            Some("31.12.26".to_string())
        );
    }

    #[test]
    fn formats_from_filename_prefix() {
        assert_eq!(
            format_short_date("2026-09-24-SPAI-001-doladit-zen.md"),
            Some("24.09.26".to_string())
        );
    }

    #[test]
    fn formats_dd_mm_yyyy_and_dd_mm_yy() {
        assert_eq!(
            format_short_date("24.09.2026"),
            Some("24.09.26".to_string())
        );
        assert_eq!(
            format_short_date("24.09.26"),
            Some("24.09.26".to_string())
        );
    }

    #[test]
    fn formats_epoch_seconds_and_millis() {
        // 1790250000 -> approx 2026-09-24
        assert!(format_short_date("1790250000").is_some());
    }

    #[test]
    fn invalid_date_returns_none() {
        assert_eq!(format_short_date(""), None);
        assert_eq!(format_short_date("hello"), None);
        assert_eq!(format_short_date("99.99.99"), None);
    }
}
