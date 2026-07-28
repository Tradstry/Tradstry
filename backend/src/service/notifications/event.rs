use chrono::NaiveDate;
use serde_json::{Value, json};

pub const ALL_EVENT_TYPES: [&str; 4] = [
    "FillsLanded",
    "BrokerageConnectionDisabled",
    "ArtifactReady",
    "PrincipleViolated",
];

#[derive(Debug, Clone, PartialEq)]
pub enum NotificationEvent {
    FillsLanded {
        account_id: String,
        broker: String,
        count: i64,
    },
    BrokerageConnectionDisabled {
        account_id: String,
        broker: String,
    },
    ArtifactReady {
        account_id: String,
        kind: String,
        artifact_id: String,
    },
    PrincipleViolated {
        account_id: String,
        trade_id: String,
        principle_id: String,
    },
}

impl NotificationEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::FillsLanded { .. } => "FillsLanded",
            Self::BrokerageConnectionDisabled { .. } => "BrokerageConnectionDisabled",
            Self::ArtifactReady { .. } => "ArtifactReady",
            Self::PrincipleViolated { .. } => "PrincipleViolated",
        }
    }

    /// `None` means the event always gets its own notification. A disabled
    /// connection needs an individually dismissable item per account, so folding
    /// two of them together would hide one broken brokerage behind another.
    pub fn coalesce_key(&self, today: NaiveDate) -> Option<String> {
        match self {
            Self::FillsLanded { account_id, .. } => Some(format!("fills:{account_id}:{today}")),
            Self::BrokerageConnectionDisabled { .. } => None,
            Self::ArtifactReady { account_id, .. } => Some(format!("artifact:{account_id}")),
            Self::PrincipleViolated { account_id, .. } => {
                Some(format!("violations:{account_id}:{today}"))
            }
        }
    }

    pub fn payload(&self) -> Value {
        match self {
            Self::FillsLanded {
                account_id,
                broker,
                count,
            } => json!({ "account_id": account_id, "broker": broker, "count": count }),
            Self::BrokerageConnectionDisabled { account_id, broker } => {
                json!({ "account_id": account_id, "broker": broker })
            }
            Self::ArtifactReady {
                account_id,
                kind,
                artifact_id,
            } => json!({ "account_id": account_id, "kind": kind, "artifact_id": artifact_id }),
            Self::PrincipleViolated {
                account_id,
                trade_id,
                principle_id,
            } => json!({
                "account_id": account_id,
                "trade_id": trade_id,
                "principle_id": principle_id
            }),
        }
    }

    pub fn account_id(&self) -> &str {
        match self {
            Self::FillsLanded { account_id, .. }
            | Self::BrokerageConnectionDisabled { account_id, .. }
            | Self::ArtifactReady { account_id, .. }
            | Self::PrincipleViolated { account_id, .. } => account_id,
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
            account_id: "acc1".into(),
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
            account_id: "acc1".into(),
            broker: "Webull".into(),
            count: 1,
        };
        let b = NotificationEvent::FillsLanded {
            account_id: "acc2".into(),
            broker: "Webull".into(),
            count: 1,
        };
        assert_ne!(a.coalesce_key(day()), b.coalesce_key(day()));
    }

    #[test]
    fn disabled_connection_is_never_grouped() {
        let e = NotificationEvent::BrokerageConnectionDisabled {
            account_id: "acc1".into(),
            broker: "Webull".into(),
        };
        assert_eq!(e.coalesce_key(day()), None);
    }

    #[test]
    fn payload_round_trips_every_field() {
        let e = NotificationEvent::ArtifactReady {
            account_id: "acc1".into(),
            kind: "ai_report".into(),
            artifact_id: "art1".into(),
        };
        let p = e.payload();
        assert_eq!(p["account_id"], "acc1");
        assert_eq!(p["kind"], "ai_report");
        assert_eq!(p["artifact_id"], "art1");
    }

    #[test]
    fn all_event_types_covers_every_variant() {
        let variants = [
            NotificationEvent::FillsLanded {
                account_id: "a".into(),
                broker: "b".into(),
                count: 1,
            },
            NotificationEvent::BrokerageConnectionDisabled {
                account_id: "a".into(),
                broker: "b".into(),
            },
            NotificationEvent::ArtifactReady {
                account_id: "a".into(),
                kind: "k".into(),
                artifact_id: "i".into(),
            },
            NotificationEvent::PrincipleViolated {
                account_id: "a".into(),
                trade_id: "t".into(),
                principle_id: "p".into(),
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
