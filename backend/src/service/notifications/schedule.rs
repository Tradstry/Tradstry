use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc};

use super::settings::UserSettings;

/// How late a tick may be and still fire. A 60s worker that misses its minute to
/// a slow query would otherwise skip the slot for the whole day; the run table
/// stops the widened window from firing twice.
pub const TOLERANCE_MINUTES: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleKind {
    DailyRecap,
    WeeklyReview,
}

impl ScheduleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DailyRecap => "DailyRecap",
            Self::WeeklyReview => "WeeklyReview",
        }
    }
}

/// The local date of the slot when `now` sits inside its firing window, else
/// `None`. Pure: every timezone and DST decision is visible to tests.
pub fn due(now: DateTime<Utc>, settings: &UserSettings, kind: ScheduleKind) -> Option<NaiveDate> {
    let local = now.with_timezone(&settings.tz());
    let local_minute = i64::from(local.hour() * 60 + local.minute());

    let scheduled = match kind {
        ScheduleKind::DailyRecap => {
            // Weekends have no close to recap.
            if local.weekday().num_days_from_monday() >= 5 {
                return None;
            }
            i64::from(settings.daily_recap_minute)
        }
        ScheduleKind::WeeklyReview => {
            if i16::try_from(local.weekday().num_days_from_sunday()).unwrap_or(-1)
                != settings.weekly_review_dow
            {
                return None;
            }
            i64::from(settings.weekly_review_minute)
        }
    };

    let delta = local_minute - scheduled;
    if (0..=TOLERANCE_MINUTES).contains(&delta) {
        Some(local.date_naive())
    } else {
        None
    }
}

/// Quiet hours as a half-open local window, correct across midnight.
pub fn in_quiet_hours(now: DateTime<Utc>, settings: &UserSettings) -> bool {
    let (Some(start), Some(end)) = (settings.quiet_start_minute, settings.quiet_end_minute) else {
        return false;
    };
    if start == end {
        return false;
    }

    let local = now.with_timezone(&settings.tz());
    let minute = i16::try_from(local.hour() * 60 + local.minute()).unwrap_or(0);

    if start < end {
        minute >= start && minute < end
    } else {
        minute >= start || minute < end
    }
}

/// When the current quiet window ends, as a UTC instant. `None` when not inside
/// one. Used to defer a push rather than drop it.
pub fn quiet_hours_end(now: DateTime<Utc>, settings: &UserSettings) -> Option<DateTime<Utc>> {
    if !in_quiet_hours(now, settings) {
        return None;
    }
    let end = settings.quiet_end_minute?;
    let tz = settings.tz();
    let local = now.with_timezone(&tz);
    let minute = i16::try_from(local.hour() * 60 + local.minute()).unwrap_or(0);

    // Past the end value means the window wraps midnight, so it lands tomorrow.
    let target_date = if minute < end {
        local.date_naive()
    } else {
        local.date_naive().succ_opt()?
    };

    let naive = target_date.and_hms_opt(u32::from(end as u16) / 60, u32::from(end as u16) % 60, 0)?;
    // A DST gap can make the wall-clock time nonexistent; the later of the pair
    // is still after the window, which is all the caller needs.
    naive
        .and_local_timezone(tz)
        .earliest()
        .or_else(|| naive.and_local_timezone(tz).latest())
        .map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn et(settings_minute: i16) -> UserSettings {
        UserSettings {
            daily_recap_minute: settings_minute,
            ..Default::default()
        }
    }

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn fires_on_the_exact_minute() {
        // 2026-07-28 is a Tuesday. 16:15 ET = 20:15 UTC in summer.
        let now = utc(2026, 7, 28, 20, 15);
        assert_eq!(
            due(now, &et(975), ScheduleKind::DailyRecap),
            Some(NaiveDate::from_ymd_opt(2026, 7, 28).unwrap())
        );
    }

    #[test]
    fn fires_one_minute_late() {
        let now = utc(2026, 7, 28, 20, 16);
        assert!(due(now, &et(975), ScheduleKind::DailyRecap).is_some());
    }

    #[test]
    fn does_not_fire_six_minutes_late() {
        let now = utc(2026, 7, 28, 20, 21);
        assert!(due(now, &et(975), ScheduleKind::DailyRecap).is_none());
    }

    #[test]
    fn does_not_fire_before_the_slot() {
        let now = utc(2026, 7, 28, 20, 14);
        assert!(due(now, &et(975), ScheduleKind::DailyRecap).is_none());
    }

    #[test]
    fn no_recap_on_saturday() {
        // 2026-08-01 is a Saturday.
        let now = utc(2026, 8, 1, 20, 15);
        assert!(due(now, &et(975), ScheduleKind::DailyRecap).is_none());
    }

    #[test]
    fn recap_uses_winter_offset_after_dst_ends() {
        // 2026-12-01 is a Tuesday; ET is UTC-5, so 16:15 local = 21:15 UTC.
        let now = utc(2026, 12, 1, 21, 15);
        assert_eq!(
            due(now, &et(975), ScheduleKind::DailyRecap),
            Some(NaiveDate::from_ymd_opt(2026, 12, 1).unwrap())
        );
        // The summer instant must not fire in winter.
        assert!(due(utc(2026, 12, 1, 20, 15), &et(975), ScheduleKind::DailyRecap).is_none());
    }

    #[test]
    fn weekly_review_fires_on_configured_day() {
        // 2026-08-02 is a Sunday. 17:00 ET = 21:00 UTC in summer.
        let s = UserSettings::default();
        assert!(due(utc(2026, 8, 2, 21, 0), &s, ScheduleKind::WeeklyReview).is_some());
        // Monday must not.
        assert!(due(utc(2026, 8, 3, 21, 0), &s, ScheduleKind::WeeklyReview).is_none());
    }

    #[test]
    fn non_us_timezone_resolves_locally() {
        let s = UserSettings {
            timezone: "Asia/Tokyo".into(),
            daily_recap_minute: 975,
            ..Default::default()
        };
        // Tokyo is UTC+9 year-round; 16:15 JST on Wed = 07:15 UTC same day.
        assert!(due(utc(2026, 7, 29, 7, 15), &s, ScheduleKind::DailyRecap).is_some());
    }

    #[test]
    fn quiet_hours_across_midnight() {
        let s = UserSettings {
            quiet_start_minute: Some(1320), // 22:00
            quiet_end_minute: Some(420),    // 07:00
            ..Default::default()
        };
        assert!(in_quiet_hours(utc(2026, 7, 29, 3, 0), &s)); // 23:00 ET
        assert!(in_quiet_hours(utc(2026, 7, 29, 9, 0), &s)); // 05:00 ET
        assert!(!in_quiet_hours(utc(2026, 7, 28, 18, 0), &s)); // 14:00 ET
    }

    #[test]
    fn quiet_hours_unset_is_never_quiet() {
        assert!(!in_quiet_hours(utc(2026, 7, 29, 3, 0), &UserSettings::default()));
    }

    #[test]
    fn quiet_end_is_in_the_future() {
        let s = UserSettings {
            quiet_start_minute: Some(1320),
            quiet_end_minute: Some(420),
            ..Default::default()
        };
        let now = utc(2026, 7, 29, 3, 0);
        let end = quiet_hours_end(now, &s).expect("inside the window");
        assert!(end > now);
    }
}
