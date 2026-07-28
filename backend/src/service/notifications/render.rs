use super::NotificationEvent;

#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub title: String,
    pub body: String,
    pub deep_link: Option<String>,
}

/// `group_count` is the running total across every event folded into this
/// notification, which is why the event's own count is never used for copy.
pub fn render(event: &NotificationEvent, group_count: i64) -> Rendered {
    match event {
        NotificationEvent::FillsLanded {
            account_id, broker, ..
        } => Rendered {
            title: if group_count == 1 {
                format!("New fill on {broker}")
            } else {
                format!("{group_count} new fills on {broker}")
            },
            body: "Review them and add them to your journal.".to_string(),
            deep_link: Some(format!("/dashboard/brokerage?account={account_id}")),
        },
        NotificationEvent::BrokerageConnectionDisabled { account_id, broker } => Rendered {
            title: format!("Reconnect {broker}"),
            body: format!("{broker} stopped syncing. Reconnect it to keep your trades up to date."),
            deep_link: Some(format!("/dashboard/brokerage?account={account_id}")),
        },
        NotificationEvent::ArtifactReady { kind, .. } => Rendered {
            title: match kind.as_str() {
                "ai_report" => "Your report is ready",
                "ai_insights" => "New insights are ready",
                "mindset_summary" => "Your mindset summary is ready",
                _ => "Your analysis is ready",
            }
            .to_string(),
            body: String::new(),
            deep_link: Some("/dashboard/analytics".to_string()),
        },
        NotificationEvent::PrincipleViolated { account_id, .. } => Rendered {
            title: if group_count == 1 {
                "A trade broke one of your principles".to_string()
            } else {
                format!("{group_count} principle violations today")
            },
            body: "Open your playbook to see which ones.".to_string(),
            deep_link: Some(format!("/dashboard/playbook?account={account_id}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::notifications::NotificationEvent;

    fn fills(count: i64) -> NotificationEvent {
        NotificationEvent::FillsLanded {
            account_id: "acc1".into(),
            broker: "Webull".into(),
            count,
        }
    }

    #[test]
    fn singular_at_group_count_one() {
        let r = render(&fills(1), 1);
        assert_eq!(r.title, "New fill on Webull");
    }

    #[test]
    fn plural_uses_the_group_count_not_the_event_count() {
        // The event carries the count from ONE sync page; group_count is the
        // running total across every folded event. Rendering the former would
        // show "1 new fill" on a notification that has absorbed twelve.
        let r = render(&fills(1), 12);
        assert_eq!(r.title, "12 new fills on Webull");
    }

    #[test]
    fn disabled_connection_links_to_the_account() {
        let r = render(
            &NotificationEvent::BrokerageConnectionDisabled {
                account_id: "acc1".into(),
                broker: "Webull".into(),
            },
            1,
        );
        assert_eq!(r.title, "Reconnect Webull");
        assert!(r.body.contains("stopped syncing"));
        assert_eq!(
            r.deep_link.as_deref(),
            Some("/dashboard/brokerage?account=acc1")
        );
    }

    #[test]
    fn artifact_titles_are_kind_specific() {
        let mk = |kind: &str| {
            render(
                &NotificationEvent::ArtifactReady {
                    account_id: "acc1".into(),
                    kind: kind.into(),
                    artifact_id: "art1".into(),
                },
                1,
            )
            .title
        };
        assert_eq!(mk("ai_report"), "Your report is ready");
        assert_eq!(mk("ai_insights"), "New insights are ready");
        assert_eq!(mk("mindset_summary"), "Your mindset summary is ready");
        assert_eq!(mk("something_new"), "Your analysis is ready");
    }

    #[test]
    fn violations_pluralize_on_group_count() {
        let e = NotificationEvent::PrincipleViolated {
            account_id: "acc1".into(),
            trade_id: "t1".into(),
            principle_id: "p1".into(),
        };
        assert_eq!(render(&e, 1).title, "A trade broke one of your principles");
        assert_eq!(render(&e, 3).title, "3 principle violations today");
    }
}
