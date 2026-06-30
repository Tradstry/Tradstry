use anyhow::{Result, anyhow};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

/// Parse a flexible date/datetime string into a UTC timestamp.
///
/// Accepts RFC3339 (with or without fractional seconds and `Z`, e.g.
/// `2026-05-13T21:21:10.401Z`), `YYYY-MM-DD HH:MM[:SS[.f]]`,
/// `YYYY-MM-DDTHH:MM[:SS[.f]]`, and bare `YYYY-MM-DD` (interpreted as midnight UTC).
///
/// This is the single source of truth for turning the legacy TEXT timestamps
/// (and incoming user/broker date strings) into the `TIMESTAMPTZ` values bound
/// into Postgres.
pub fn parse_flexible_datetime(value: &str) -> Result<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Ok(parsed.with_timezone(&Utc));
    }

    for format in [
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(value, format) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(parsed, Utc));
        }
    }

    if let Ok(parsed) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let midnight = parsed
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow!("Invalid date provided"))?;
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(midnight, Utc));
    }

    Err(anyhow!(
        "Invalid datetime format. Use RFC3339, YYYY-MM-DD HH:MM[:SS], or YYYY-MM-DD"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3339_with_millis() {
        let dt = parse_flexible_datetime("2026-05-13T21:21:10.401Z").unwrap();
        assert_eq!(
            dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "2026-05-13T21:21:10.401Z"
        );
    }

    #[test]
    fn parses_bare_date() {
        let dt = parse_flexible_datetime("2026-01-02").unwrap();
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-01-02 00:00:00"
        );
    }
}
