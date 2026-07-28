use anyhow::{Context, Result};
use chrono_tz::Tz;
use sqlx::PgPool;
use std::str::FromStr;

pub const DEFAULT_TIMEZONE: &str = "America/New_York";
pub const DEFAULT_DAILY_RECAP_MINUTE: i16 = 975;
pub const DEFAULT_WEEKLY_REVIEW_DOW: i16 = 0;
pub const DEFAULT_WEEKLY_REVIEW_MINUTE: i16 = 1020;

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct UserSettings {
    pub timezone: String,
    pub daily_recap_minute: i16,
    pub weekly_review_dow: i16,
    pub weekly_review_minute: i16,
    pub quiet_start_minute: Option<i16>,
    pub quiet_end_minute: Option<i16>,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            timezone: DEFAULT_TIMEZONE.to_string(),
            daily_recap_minute: DEFAULT_DAILY_RECAP_MINUTE,
            weekly_review_dow: DEFAULT_WEEKLY_REVIEW_DOW,
            weekly_review_minute: DEFAULT_WEEKLY_REVIEW_MINUTE,
            quiet_start_minute: None,
            quiet_end_minute: None,
        }
    }
}

impl UserSettings {
    /// Reads tolerate a bad zone rather than failing the whole tick; input
    /// validation happens at the GraphQL boundary instead.
    pub fn tz(&self) -> Tz {
        Tz::from_str(&self.timezone).unwrap_or(chrono_tz::America::New_York)
    }
}

#[derive(Debug, Default, Clone)]
pub struct SettingsPatch {
    pub timezone: Option<String>,
    pub daily_recap_minute: Option<i16>,
    pub weekly_review_dow: Option<i16>,
    pub weekly_review_minute: Option<i16>,
    pub quiet_start_minute: Option<Option<i16>>,
    pub quiet_end_minute: Option<Option<i16>>,
}

/// A missing row means defaults — the same convention the preferences table
/// uses, and for the same reason: a new user needs no backfill.
pub async fn get(pool: &PgPool, user_id: &str) -> Result<UserSettings> {
    let row: Option<UserSettings> = sqlx::query_as(
        "SELECT timezone, daily_recap_minute, weekly_review_dow, weekly_review_minute, \
                quiet_start_minute, quiet_end_minute \
         FROM notification_user_settings WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("failed to read notification settings")?;

    Ok(row.unwrap_or_default())
}

pub async fn upsert(pool: &PgPool, user_id: &str, patch: &SettingsPatch) -> Result<UserSettings> {
    let current = get(pool, user_id).await?;

    let next = UserSettings {
        timezone: patch.timezone.clone().unwrap_or(current.timezone),
        daily_recap_minute: patch
            .daily_recap_minute
            .unwrap_or(current.daily_recap_minute),
        weekly_review_dow: patch.weekly_review_dow.unwrap_or(current.weekly_review_dow),
        weekly_review_minute: patch
            .weekly_review_minute
            .unwrap_or(current.weekly_review_minute),
        quiet_start_minute: patch
            .quiet_start_minute
            .unwrap_or(current.quiet_start_minute),
        quiet_end_minute: patch.quiet_end_minute.unwrap_or(current.quiet_end_minute),
    };

    sqlx::query(
        "INSERT INTO notification_user_settings \
           (user_id, timezone, daily_recap_minute, weekly_review_dow, weekly_review_minute, \
            quiet_start_minute, quiet_end_minute) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (user_id) DO UPDATE SET \
           timezone = EXCLUDED.timezone, \
           daily_recap_minute = EXCLUDED.daily_recap_minute, \
           weekly_review_dow = EXCLUDED.weekly_review_dow, \
           weekly_review_minute = EXCLUDED.weekly_review_minute, \
           quiet_start_minute = EXCLUDED.quiet_start_minute, \
           quiet_end_minute = EXCLUDED.quiet_end_minute",
    )
    .bind(user_id)
    .bind(&next.timezone)
    .bind(next.daily_recap_minute)
    .bind(next.weekly_review_dow)
    .bind(next.weekly_review_minute)
    .bind(next.quiet_start_minute)
    .bind(next.quiet_end_minute)
    .execute(pool)
    .await
    .context("failed to write notification settings")?;

    Ok(next)
}

pub fn is_valid_timezone(name: &str) -> bool {
    Tz::from_str(name).is_ok()
}

pub fn is_valid_minute(minute: i16) -> bool {
    (0..=1439).contains(&minute)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_zone_falls_back_to_eastern() {
        let s = UserSettings {
            timezone: "Mars/Olympus_Mons".into(),
            ..Default::default()
        };
        assert_eq!(s.tz(), chrono_tz::America::New_York);
    }

    #[test]
    fn known_zone_is_used() {
        let s = UserSettings {
            timezone: "Asia/Tokyo".into(),
            ..Default::default()
        };
        assert_eq!(s.tz(), chrono_tz::Asia::Tokyo);
    }

    #[test]
    fn defaults_are_the_documented_slots() {
        let s = UserSettings::default();
        assert_eq!(s.daily_recap_minute, 975);
        assert_eq!(s.weekly_review_dow, 0);
        assert_eq!(s.weekly_review_minute, 1020);
        assert!(s.quiet_start_minute.is_none());
    }

    #[test]
    fn minute_bounds() {
        assert!(is_valid_minute(0));
        assert!(is_valid_minute(1439));
        assert!(!is_valid_minute(1440));
        assert!(!is_valid_minute(-1));
    }
}
