use super::NotificationEvent;
use super::metrics::{self, WeeklyStats};

#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub title: String,
    pub body: String,
    pub deep_link: Option<String>,
}

/// Blocks that fail their sample-size gate are absent from `stats` and leave no
/// trace here — no "not enough data" filler. The one exception is setup
/// progress, where the shortfall is itself the message.
///
/// P&L never appears. Performance feedback provokes asymmetric risk-taking
/// (NBER w22146), so the push stays on process and the app keeps the numbers.
/// The ratio is losers-over-winners, so a value below 1 means losers were cut
/// *faster* — the healthy direction. Phrasing that case as "0.8x longer" reads
/// as a problem, so it inverts instead. Inside the neutral band neither side
/// gets a directional claim.
fn asymmetry_copy(ratio: f64) -> String {
    if ratio > metrics::NEUTRAL_BAND_HIGH {
        format!("You held losers {ratio:.1}x longer than winners over the last 90 days")
    } else if ratio < metrics::NEUTRAL_BAND_LOW && ratio > 0.0 {
        format!(
            "You held winners {:.1}x longer than losers over the last 90 days",
            1.0 / ratio
        )
    } else {
        "You held winners and losers about the same length of time over the last 90 days"
            .to_string()
    }
}

fn weekly_body(stats: &WeeklyStats) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(counts) = &stats.counts
        && counts.trades > 0
    {
        parts.push(format!(
            "{} of {} trades journaled.",
            counts.journaled, counts.trades
        ));

        if counts.violations > 0 {
            let mut line = format!(
                "{} of {} broke a principle.",
                counts.violations, counts.trades
            );
            if let Some(title) = &counts.top_principle {
                line.push_str(&format!(
                    " Most often: \"{}\" ({}).",
                    title, counts.top_principle_count
                ));
            }
            parts.push(line);
        }
    }

    if let Some(a) = &stats.asymmetry {
        parts.push(format!(
            "{} ({} wins, {} losses).",
            asymmetry_copy(a.ratio),
            a.wins,
            a.losses
        ));
    }

    for setup in &stats.setups {
        parts.push(format!(
            "{} — {} of {} trades. Not enough yet to tell signal from luck.",
            setup.name, setup.closed, setup.target
        ));
    }

    parts.join(" ")
}

/// `group_count` is the running total across every event folded into this
/// notification, which is why the event's own count is never used for copy.
pub fn render(event: &NotificationEvent, group_count: i64) -> Rendered {
    match event {
        NotificationEvent::FillsLanded {
            workspace_id,
            broker,
            ..
        } => Rendered {
            title: if group_count == 1 {
                format!("New fill on {broker}")
            } else {
                format!("{group_count} new fills on {broker}")
            },
            body: "Review them and add them to your journal.".to_string(),
            deep_link: Some(format!("/dashboard/brokerage?account={workspace_id}")),
        },
        NotificationEvent::BrokerageConnectionDisabled {
            workspace_id,
            broker,
        } => Rendered {
            title: format!("Reconnect {broker}"),
            body: format!("{broker} stopped syncing. Reconnect it to keep your trades up to date."),
            deep_link: Some(format!("/dashboard/brokerage?account={workspace_id}")),
        },
        NotificationEvent::ArtifactReady { kind, .. } => Rendered {
            title: match kind.as_str() {
                "ai_report" => "Your report is ready",
                "ai_insights" => "New insights are ready",
                "mindset_summary" => "Your mindset summary is ready",
                "market_report" => "Your market report is ready",
                _ => "Your analysis is ready",
            }
            .to_string(),
            body: String::new(),
            deep_link: Some(if kind == "market_report" {
                "/dashboard/markets?tab=reports".to_string()
            } else {
                "/dashboard/analytics".to_string()
            }),
        },
        NotificationEvent::PrincipleViolated { workspace_id, .. } => Rendered {
            title: if group_count == 1 {
                "A trade broke one of your principles".to_string()
            } else {
                format!("{group_count} principle violations today")
            },
            body: "Open your playbook to see which ones.".to_string(),
            deep_link: Some(format!("/dashboard/playbook?account={workspace_id}")),
        },
        NotificationEvent::DailyRecap {
            workspace_id,
            symbol_count,
            ..
        } => Rendered {
            title: if *symbol_count == 1 {
                "1 symbol to journal".to_string()
            } else {
                format!("{symbol_count} symbols to journal")
            },
            body: "Traded today and not written up yet. The details fade fast.".to_string(),
            deep_link: Some(format!("/dashboard/brokerage?account={workspace_id}")),
        },
        NotificationEvent::WeeklyReview { stats, .. } => Rendered {
            title: "Your week in review".to_string(),
            body: weekly_body(stats),
            deep_link: Some("/dashboard/analytics".to_string()),
        },
        NotificationEvent::MarketMonitorTriggered {
            workspace_id,
            symbol,
            monitor_name,
            price,
        } => Rendered {
            title: format!("{symbol} market alert"),
            body: format!("{monitor_name}: {symbol} is now ${price:.2}."),
            deep_link: Some(format!(
                "/dashboard/markets?workspace={workspace_id}&symbol={symbol}"
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::notifications::NotificationEvent;
    use crate::service::notifications::metrics;

    fn day() -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()
    }

    fn fills(count: i64) -> NotificationEvent {
        NotificationEvent::FillsLanded {
            workspace_id: "acc1".into(),
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
                workspace_id: "acc1".into(),
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
                    workspace_id: "acc1".into(),
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
            workspace_id: "acc1".into(),
            trade_id: "t1".into(),
            principle_id: "p1".into(),
        };
        assert_eq!(render(&e, 1).title, "A trade broke one of your principles");
        assert_eq!(render(&e, 3).title, "3 principle violations today");
    }

    fn counts(trades: i64, journaled: i64, violations: i64) -> metrics::WeeklyCounts {
        metrics::WeeklyCounts {
            trades,
            journaled,
            violations,
            top_principle: None,
            top_principle_count: 0,
        }
    }

    fn review(stats: WeeklyStats) -> Rendered {
        render(
            &NotificationEvent::WeeklyReview {
                workspace_id: "acc1".into(),
                iso_week: "2026-W31".into(),
                stats,
            },
            1,
        )
    }

    #[test]
    fn recap_pluralizes_on_symbol_count() {
        let mk = |n: i64| {
            render(
                &NotificationEvent::DailyRecap {
                    workspace_id: "acc1".into(),
                    local_date: day(),
                    symbol_count: n,
                },
                1,
            )
            .title
        };
        assert_eq!(mk(1), "1 symbol to journal");
        assert_eq!(mk(4), "4 symbols to journal");
    }

    #[test]
    fn review_omits_absent_blocks_without_filler() {
        let body = review(WeeklyStats {
            counts: Some(counts(10, 8, 0)),
            asymmetry: None,
            setups: vec![],
        })
        .body;
        assert_eq!(body, "8 of 10 trades journaled.");
        assert!(!body.contains("not enough"));
        assert!(!body.contains("N/A"));
    }

    #[test]
    fn review_names_the_most_broken_principle() {
        let body = review(WeeklyStats {
            counts: Some(metrics::WeeklyCounts {
                top_principle: Some("No adds to a loser".into()),
                top_principle_count: 2,
                ..counts(12, 12, 3)
            }),
            ..Default::default()
        })
        .body;
        assert!(body.contains("3 of 12 broke a principle."));
        assert!(body.contains("\"No adds to a loser\" (2)"));
    }

    #[test]
    fn review_reports_asymmetry_as_a_ratio() {
        let body = review(WeeklyStats {
            asymmetry: Some(metrics::Asymmetry {
                ratio: 2.4,
                wins: 18,
                losses: 21,
            }),
            ..Default::default()
        })
        .body;
        assert!(body.contains("held losers 2.4x longer than winners"));
        assert!(body.contains("18 wins, 21 losses"));
    }

    /// Below 1 the trader is cutting losers faster than winners — the healthy
    /// direction. Reporting it as "0.8x longer" reads as a fault.
    #[test]
    fn a_ratio_below_one_inverts_instead_of_reading_as_a_fault() {
        let copy = asymmetry_copy(0.5);
        assert!(
            copy.contains("held winners 2.0x longer than losers"),
            "got {copy:?}"
        );
        assert!(!copy.contains("0.5"));
    }

    #[test]
    fn the_neutral_band_makes_no_directional_claim() {
        for ratio in [0.9, 0.95, 1.0, 1.05, 1.1] {
            let copy = asymmetry_copy(ratio);
            assert!(
                copy.contains("about the same length"),
                "{ratio} should be neutral, got {copy:?}"
            );
            assert!(!copy.contains("longer than"), "{ratio} claimed a direction");
        }
    }

    #[test]
    fn the_band_edges_are_directional() {
        assert!(asymmetry_copy(1.11).contains("held losers"));
        assert!(asymmetry_copy(0.89).contains("held winners"));
    }

    /// A zero median winner is already filtered upstream, but a divide-by-zero
    /// here would render "infx longer".
    #[test]
    fn a_zero_ratio_stays_neutral() {
        assert!(asymmetry_copy(0.0).contains("about the same length"));
    }

    #[test]
    fn setup_progress_states_the_shortfall() {
        let body = review(WeeklyStats {
            setups: vec![metrics::SetupProgress {
                name: "Breakout".into(),
                closed: 23,
                target: 100,
            }],
            ..Default::default()
        })
        .body;
        assert!(body.contains("Breakout — 23 of 100 trades."));
    }

    /// The whole point of the design: process only, never performance.
    #[test]
    fn no_scheduled_copy_carries_a_pnl_figure() {
        let bodies = [
            review(WeeklyStats {
                counts: Some(counts(12, 12, 3)),
                asymmetry: Some(metrics::Asymmetry {
                    ratio: 2.4,
                    wins: 18,
                    losses: 21,
                }),
                setups: vec![metrics::SetupProgress {
                    name: "Breakout".into(),
                    closed: 23,
                    target: 100,
                }],
            })
            .body,
            render(
                &NotificationEvent::DailyRecap {
                    workspace_id: "acc1".into(),
                    local_date: day(),
                    symbol_count: 3,
                },
                1,
            )
            .body,
        ];

        for body in bodies {
            for banned in ['$', '€', '£', '%'] {
                assert!(!body.contains(banned), "{body:?} leaked {banned}");
            }
            for banned in ["P&L", "profit", "return", "R:R"] {
                assert!(!body.contains(banned), "{body:?} leaked {banned:?}");
            }
        }
    }
}
