use chrono::NaiveDate;
use serde_json::{Value, json};

use super::metrics::WeeklyStats;

pub const ALL_EVENT_TYPES: [&str; 6] = [
    "FillsLanded",
    "BrokerageConnectionDisabled",
    "ArtifactReady",
    "PrincipleViolated",
    "DailyRecap",
    "WeeklyReview",
];

#[derive(Debug, Clone, PartialEq)]
pub enum NotificationEvent {
    FillsLanded {
        workspace_id: String,
        broker: String,
        count: i64,
    },
    BrokerageConnectionDisabled {
        workspace_id: String,
        broker: String,
    },
    ArtifactReady {
        workspace_id: String,
        kind: String,
        artifact_id: String,
    },
    PrincipleViolated {
        workspace_id: String,
        trade_id: String,
        principle_id: String,
    },
    /// Scheduled. Metrics are computed by the scheduler and carried here so the
    /// renderer stays a pure function with no database access.
    DailyRecap {
        workspace_id: String,
        local_date: NaiveDate,
        symbol_count: i64,
    },
    WeeklyReview {
        workspace_id: String,
        iso_week: String,
        stats: WeeklyStats,
    },
}

impl NotificationEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::FillsLanded { .. } => "FillsLanded",
            Self::BrokerageConnectionDisabled { .. } => "BrokerageConnectionDisabled",
            Self::ArtifactReady { .. } => "ArtifactReady",
            Self::PrincipleViolated { .. } => "PrincipleViolated",
            Self::DailyRecap { .. } => "DailyRecap",
            Self::WeeklyReview { .. } => "WeeklyReview",
        }
    }

    /// `None` means the event always gets its own notification. A disabled
    /// connection needs an individually dismissable item per account, so folding
    /// two of them together would hide one broken brokerage behind another.
    pub fn coalesce_key(&self, today: NaiveDate) -> Option<String> {
        match self {
            Self::FillsLanded { workspace_id, .. } => Some(format!("fills:{workspace_id}:{today}")),
            Self::BrokerageConnectionDisabled { .. } => None,
            Self::ArtifactReady { workspace_id, .. } => Some(format!("artifact:{workspace_id}")),
            Self::PrincipleViolated { workspace_id, .. } => {
                Some(format!("violations:{workspace_id}:{today}"))
            }
            Self::DailyRecap { local_date, .. } => Some(format!("recap:{local_date}")),
            Self::WeeklyReview { iso_week, .. } => Some(format!("review:{iso_week}")),
        }
    }

    pub fn payload(&self) -> Value {
        match self {
            Self::FillsLanded {
                workspace_id,
                broker,
                count,
            } => json!({ "workspace_id": workspace_id, "broker": broker, "count": count }),
            Self::BrokerageConnectionDisabled {
                workspace_id,
                broker,
            } => {
                json!({ "workspace_id": workspace_id, "broker": broker })
            }
            Self::ArtifactReady {
                workspace_id,
                kind,
                artifact_id,
            } => json!({ "workspace_id": workspace_id, "kind": kind, "artifact_id": artifact_id }),
            Self::PrincipleViolated {
                workspace_id,
                trade_id,
                principle_id,
            } => json!({
                "workspace_id": workspace_id,
                "trade_id": trade_id,
                "principle_id": principle_id
            }),
            Self::DailyRecap {
                workspace_id,
                local_date,
                symbol_count,
            } => json!({
                "workspace_id": workspace_id,
                "local_date": local_date.to_string(),
                "symbol_count": symbol_count
            }),
            Self::WeeklyReview {
                workspace_id,
                iso_week,
                stats,
            } => json!({
                "workspace_id": workspace_id,
                "iso_week": iso_week,
                "stats": stats
            }),
        }
    }

    pub fn workspace_id(&self) -> &str {
        match self {
            Self::FillsLanded { workspace_id, .. }
            | Self::BrokerageConnectionDisabled { workspace_id, .. }
            | Self::ArtifactReady { workspace_id, .. }
            | Self::PrincipleViolated { workspace_id, .. }
            | Self::DailyRecap { workspace_id, .. }
            | Self::WeeklyReview { workspace_id, .. } => workspace_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()
    }

    #[test]
    fn fills_group_per_account_per_day() {
        let e = NotificationEvent::FillsLanded {
            workspace_id: "acc1".into(),
            broker: "Webull".into(),
            count: 12,
        };
        assert_eq!(e.event_type(), "FillsLanded");
        assert_eq!(
            e.coalesce_key(day()).as_deref(),
            Some("fills:acc1:2026-07-28")
        );
    }

    #[test]
    fn two_accounts_do_not_share_a_group() {
        let a = NotificationEvent::FillsLanded {
            workspace_id: "acc1".into(),
            broker: "Webull".into(),
            count: 1,
        };
        let b = NotificationEvent::FillsLanded {
            workspace_id: "acc2".into(),
            broker: "Webull".into(),
            count: 1,
        };
        assert_ne!(a.coalesce_key(day()), b.coalesce_key(day()));
    }

    #[test]
    fn disabled_connection_is_never_grouped() {
        let e = NotificationEvent::BrokerageConnectionDisabled {
            workspace_id: "acc1".into(),
            broker: "Webull".into(),
        };
        assert_eq!(e.coalesce_key(day()), None);
    }

    #[test]
    fn payload_round_trips_every_field() {
        let e = NotificationEvent::ArtifactReady {
            workspace_id: "acc1".into(),
            kind: "ai_report".into(),
            artifact_id: "art1".into(),
        };
        let p = e.payload();
        assert_eq!(p["workspace_id"], "acc1");
        assert_eq!(p["kind"], "ai_report");
        assert_eq!(p["artifact_id"], "art1");
    }

    #[test]
    fn all_event_types_covers_every_variant() {
        let variants = [
            NotificationEvent::FillsLanded {
                workspace_id: "a".into(),
                broker: "b".into(),
                count: 1,
            },
            NotificationEvent::BrokerageConnectionDisabled {
                workspace_id: "a".into(),
                broker: "b".into(),
            },
            NotificationEvent::ArtifactReady {
                workspace_id: "a".into(),
                kind: "k".into(),
                artifact_id: "i".into(),
            },
            NotificationEvent::PrincipleViolated {
                workspace_id: "a".into(),
                trade_id: "t".into(),
                principle_id: "p".into(),
            },
            NotificationEvent::DailyRecap {
                workspace_id: "a".into(),
                local_date: day(),
                symbol_count: 3,
            },
            NotificationEvent::WeeklyReview {
                workspace_id: "a".into(),
                iso_week: "2026-W31".into(),
                stats: Default::default(),
            },
        ];
        for v in &variants {
            assert!(
                ALL_EVENT_TYPES.contains(&v.event_type()),
                "{} missing from ALL_EVENT_TYPES",
                v.event_type()
            );
        }
        assert_eq!(ALL_EVENT_TYPES.len(), variants.len());
    }
}
