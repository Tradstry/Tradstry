use anyhow::{Context, Result};

use sqlx::PgPool;

use super::client::{
    BrokerageClient, HoldingsSyncStatus, SnapTradeActivity, SnapTradePosition,
    TransactionsSyncStatus,
};
use crate::service::db::schema::tables::brokerage_table::{
    self, NewBrokerageBalance, NewBrokerageHolding, NewBrokerageTransaction,
};

// Fetch full history so backdated or amended fills are not missed. Upserts by
// `dedup_key` make overlapping pages idempotent.

/// Empty results may advance only when the account has no stored transactions;
/// re-registration can report a watermark while history is still backfilling.
fn should_advance_watermark(synced: u64, stored: i64) -> bool {
    synced > 0 || stored == 0
}

/// Fetches transactions when SnapTrade's daily watermark advances. Missing or
/// invalid status fails open. `force` is for manual syncs only; scheduled forcing
/// cannot provide intraday fills, which come from the holdings orders feed.
// Each argument represents a distinct upstream identity or credential.
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    skip_all,
    fields(st_account = %snaptrade_account_id, account = %internal_account_id, force)
)]
pub async fn sync_transactions_if_advanced(
    client: &BrokerageClient,
    pool: &PgPool,
    snaptrade_user_id: &str,
    user_secret: &str,
    snaptrade_account_id: &str,
    internal_user_id: &str,
    internal_account_id: &str,
    broker: &str,
    remote: Option<&TransactionsSyncStatus>,
    force: bool,
) -> Result<Option<u64>> {
    // Wait for backfill rather than storing a partial history.
    if let Some(status) = remote
        && status.initial_sync_completed == Some(false)
    {
        log::info!(
            "SnapTrade initial transaction sync still running for st_account={snaptrade_account_id}; skipping"
        );
        return Ok(None);
    }

    let remote_mark = remote.and_then(|s| s.last_successful_sync.as_deref());

    if !force && let Some(remote_mark) = remote_mark {
        let local_mark = brokerage_table::transactions_synced_through(
            pool,
            internal_user_id,
            internal_account_id,
            snaptrade_account_id,
        )
        .await?;

        // Day-granular ISO dates, so lexical ordering is chronological.
        if local_mark.as_deref() >= Some(remote_mark) {
            log::info!(
                "SnapTrade has no new transactions for st_account={snaptrade_account_id} \
                 (synced through {remote_mark}); skipping fetch"
            );
            return Ok(None);
        }
    }

    let held_before =
        brokerage_table::count_transactions(pool, internal_user_id, internal_account_id).await?;

    let synced = sync_transactions(
        client,
        pool,
        snaptrade_user_id,
        user_secret,
        snaptrade_account_id,
        internal_user_id,
        internal_account_id,
    )
    .await?;

    // Upsert counts include re-reads; the row delta identifies new fills.
    let stored =
        brokerage_table::count_transactions(pool, internal_user_id, internal_account_id).await?;
    let landed = stored.saturating_sub(held_before);
    log::info!(
        "SnapTrade fetch for st_account={snaptrade_account_id} (force={force}): {synced} fill(s) \
         processed, {landed} new row(s) — held {held_before}, now {stored}"
    );

    // Notify only for newly stored rows, not full-history re-reads.
    if landed > 0 {
        let event = crate::service::notifications::NotificationEvent::FillsLanded {
            workspace_id: internal_account_id.to_string(),
            broker: broker.to_string(),
            count: landed,
        };
        let today = chrono::Utc::now()
            .with_timezone(&chrono_tz::US::Eastern)
            .date_naive();
        if let Err(e) =
            crate::service::notifications::outbox::record(pool, internal_user_id, &event, today)
                .await
        {
            log::warn!("failed to record fills event for account={internal_account_id}: {e}");
        }
    }

    // Advance only after a successful fetch.
    if let Some(remote_mark) = remote_mark {
        // Retry empty results when existing rows suggest an incomplete backfill.
        if !should_advance_watermark(synced, stored) {
            log::warn!(
                "SnapTrade returned no transactions for st_account={snaptrade_account_id} while \
                 claiming to have synced through {remote_mark}, but we hold {stored} rows — \
                 not advancing the watermark so the next run retries"
            );
        } else {
            brokerage_table::record_transactions_synced_through(
                pool,
                internal_user_id,
                internal_account_id,
                snaptrade_account_id,
                remote_mark,
            )
            .await?;
        }
    }

    Ok(Some(synced))
}

/// Syncs transactions from SnapTrade.
/// SnapTrade IDs are for API calls; internal IDs are for database access. The
/// namespaces are not guaranteed to match.
pub async fn sync_transactions(
    client: &BrokerageClient,
    pool: &PgPool,
    snaptrade_user_id: &str,
    user_secret: &str,
    snaptrade_account_id: &str,
    internal_user_id: &str,
    internal_account_id: &str,
) -> Result<u64> {
    let mut total_synced = 0u64;
    let mut offset = 0i32;
    let limit = 1000i32;
    // Preserve partial-fill ordinals across page boundaries.
    let mut seen = brokerage_table::SignatureCounts::new();

    log::info!("Full-history sync for account={internal_account_id}");

    loop {
        let response = client
            .fetch_transactions(
                snaptrade_user_id,
                user_secret,
                snaptrade_account_id,
                None,
                None,
                None,
                Some(offset),
                Some(limit),
            )
            .await
            .context("Failed to fetch transactions page")?;

        let activities = &response.data;
        if activities.is_empty() {
            break;
        }

        let new_txs: Vec<NewBrokerageTransaction> = activities
            .iter()
            .filter_map(map_activity_to_transaction)
            .collect();

        let upserted = brokerage_table::upsert_transactions(
            pool,
            internal_user_id,
            internal_account_id,
            &new_txs,
            &mut seen,
        )
        .await
        .context("Failed to upsert transactions")?;
        total_synced += upserted;

        let page_total = response
            .pagination
            .as_ref()
            .and_then(|p| p.total)
            .unwrap_or(activities.len() as i32);

        offset += limit;
        if offset >= page_total {
            break;
        }
    }

    if total_synced > 0
        && let Err(e) = crate::service::equity::rebuild::rebuild_account_equity(
            pool,
            internal_user_id,
            internal_account_id,
        )
        .await
    {
        log::warn!("equity: rebuild after transaction sync failed: {e}");
    }

    Ok(total_synced)
}

/// Syncs holdings from SnapTrade.
/// SnapTrade IDs are for API calls; internal IDs are for database access.
/// `skip_all` also protects credentials added to this function later.
#[tracing::instrument(
    skip_all,
    fields(st_account = %snaptrade_account_id, account = %internal_account_id)
)]
pub async fn sync_holdings(
    client: &BrokerageClient,
    pool: &PgPool,
    snaptrade_user_id: &str,
    user_secret: &str,
    snaptrade_account_id: &str,
    internal_user_id: &str,
    internal_account_id: &str,
) -> Result<(u64, u64)> {
    let response = client
        .fetch_holdings(snaptrade_user_id, user_secret, snaptrade_account_id)
        .await
        .context("Failed to fetch holdings")?;

    // Orders are SnapTrade's only intraday activity source; record availability.
    tracing::info!(
        positions = response.positions.len(),
        orders = response.orders.len(),
        complete = response.complete,
        holdings_unavailable = response.holdings_unavailable,
        as_of = response.as_of.as_deref().unwrap_or_default(),
        "SnapTrade portfolio fetched"
    );

    if response.holdings_unavailable {
        if let Some(tv) = &response.total_value
            && let Some(amount) = tv.amount
        {
            crate::service::db::schema::tables::workspaces_table::update_total_value(
                pool,
                internal_account_id,
                internal_user_id,
                amount,
                tv.currency.as_deref(),
            )
            .await?;
        }
        tracing::warn!(
            "SnapTrade reports holdings unavailable; preserving the last complete local snapshot"
        );
        return Ok((0, 0));
    }
    anyhow::ensure!(
        response.complete,
        "SnapTrade adapter returned an incomplete portfolio snapshot"
    );

    let mut holdings: Vec<NewBrokerageHolding> = Vec::new();

    for position in &response.positions {
        if let Some(holding) = map_position_to_holding(position) {
            holdings.push(holding);
        }
    }

    let balances: Vec<NewBrokerageBalance> = response
        .balances
        .iter()
        .map(|balance| NewBrokerageBalance {
            currency: if balance.currency.is_empty() {
                "USD".to_string()
            } else {
                balance.currency.clone()
            },
            cash: balance.cash,
            buying_power: balance.buying_power,
        })
        .collect();

    let (holdings_count, balances_count) = brokerage_table::replace_portfolio_snapshot(
        pool,
        internal_user_id,
        internal_account_id,
        &holdings,
        &balances,
    )
    .await?;

    // Store authoritative total value for equity-based analytics when available.
    if let Some(tv) = &response.total_value
        && let Some(amount) = tv.amount
    {
        crate::service::db::schema::tables::workspaces_table::update_total_value(
            pool,
            internal_account_id,
            internal_user_id,
            amount,
            tv.currency.as_deref(),
        )
        .await?;
    }

    Ok((holdings_count, balances_count))
}

#[allow(clippy::too_many_arguments)]
pub async fn sync_holdings_if_advanced(
    client: &BrokerageClient,
    pool: &PgPool,
    snaptrade_user_id: &str,
    user_secret: &str,
    snaptrade_account_id: &str,
    internal_user_id: &str,
    internal_account_id: &str,
    remote: Option<&HoldingsSyncStatus>,
    data_freshness_mode: &str,
    force: bool,
) -> Result<Option<(u64, u64)>> {
    if remote.is_some_and(|status| status.initial_sync_completed == Some(false)) {
        return Ok(None);
    }
    let remote_mark = remote.and_then(|status| status.last_successful_sync.as_deref());
    if !force
        && data_freshness_mode == "delayed"
        && let Some(remote_mark) = remote_mark
    {
        let local_mark = brokerage_table::holdings_synced_through(
            pool,
            internal_user_id,
            internal_account_id,
            snaptrade_account_id,
        )
        .await?;
        if local_mark.as_deref() >= Some(remote_mark) {
            return Ok(None);
        }
    }

    let result = sync_holdings(
        client,
        pool,
        snaptrade_user_id,
        user_secret,
        snaptrade_account_id,
        internal_user_id,
        internal_account_id,
    )
    .await?;
    if let Some(remote_mark) = remote_mark {
        brokerage_table::record_holdings_synced_through(
            pool,
            internal_user_id,
            internal_account_id,
            snaptrade_account_id,
            remote_mark,
        )
        .await?;
    }
    Ok(Some(result))
}

fn map_activity_to_transaction(a: &SnapTradeActivity) -> Option<NewBrokerageTransaction> {
    let snaptrade_id = a.id.clone()?;
    let settlement_date = a.settlement_date.clone().unwrap_or_default();
    let institution = a.institution.clone().unwrap_or_default();
    let transaction_type = a.activity_type.clone().unwrap_or_default();
    let raw_json = serde_json::to_string(a).unwrap_or_default();

    // Options use `option_symbol`; equities use the top-level `symbol`.
    let opt = a.option_symbol.as_ref();

    let (
        symbol,
        symbol_description,
        contract_multiplier,
        underlying_symbol,
        option_kind,
        strike_price,
        option_expiration,
    ) = match &opt {
        Some(o) => {
            let underlying = o.underlying_symbol.as_ref().and_then(|s| s.symbol.clone());
            let option_kind = o.option_type.clone();
            let description = format_option_description(
                underlying.as_deref(),
                option_kind.as_deref(),
                o.strike_price,
                o.expiration_date.as_deref(),
            );
            (
                o.ticker.clone(),
                Some(description),
                if o.is_mini_option.unwrap_or(false) {
                    10.0
                } else {
                    100.0
                },
                underlying,
                option_kind,
                o.strike_price,
                o.expiration_date.clone(),
            )
        }
        None => (
            a.symbol.as_ref().and_then(|s| s.symbol.clone()),
            a.symbol.as_ref().and_then(|s| s.description.clone()),
            1.0,
            None,
            None,
            None,
            None,
        ),
    };

    Some(NewBrokerageTransaction {
        snaptrade_id,
        symbol,
        symbol_description,
        raw_symbol: a.symbol.as_ref().and_then(|s| s.raw_symbol.clone()),
        currency: a
            .currency
            .as_ref()
            .and_then(|c| c.code.clone())
            .unwrap_or_else(|| "USD".to_string()),
        transaction_type,
        option_type: a.option_type.clone(),
        price: a.price.unwrap_or(0.0),
        units: a.units.unwrap_or(0.0),
        amount: a.amount,
        fee: a.fee.unwrap_or(0.0),
        fx_rate: a.fx_rate,
        description: a.description.clone(),
        trade_date: a.trade_date.clone(),
        settlement_date,
        institution,
        external_reference_id: a.external_reference_id.clone(),
        raw_json,
        contract_multiplier,
        underlying_symbol,
        option_kind,
        strike_price,
        option_expiration,
    })
}

/// Human-readable option contract label, e.g. `AAPL $150 Call 2026-01-17`.
/// Missing parts are skipped so a partial contract still renders cleanly.
fn format_option_description(
    underlying: Option<&str>,
    kind: Option<&str>,
    strike: Option<f64>,
    expiration: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(u) = underlying {
        parts.push(u.to_string());
    }
    if let Some(s) = strike {
        parts.push(format!("${s}"));
    }
    if let Some(k) = kind {
        let pretty = match k.to_ascii_uppercase().as_str() {
            "CALL" => "Call".to_string(),
            "PUT" => "Put".to_string(),
            other => other.to_string(),
        };
        parts.push(pretty);
    }
    if let Some(e) = expiration {
        parts.push(e.to_string());
    }
    parts.join(" ")
}

fn map_position_to_holding(p: &SnapTradePosition) -> Option<NewBrokerageHolding> {
    if p.symbol.is_empty() {
        return None;
    }
    let raw_json = serde_json::to_string(p).unwrap_or_default();
    let option = p.option.as_ref();

    Some(NewBrokerageHolding {
        snaptrade_symbol_id: Some(p.instrument_id.clone()),
        symbol: p.symbol.clone(),
        symbol_description: p.description.clone(),
        raw_symbol: p.raw_symbol.clone(),
        currency: p.currency.clone().unwrap_or_else(|| "USD".to_string()),
        units: p.units.unwrap_or(0.0),
        price: p.price.unwrap_or(0.0),
        market_value: p.units.zip(p.price).map(|(units, price)| units * price),
        open_pnl: None,
        average_purchase_price: p.average_purchase_price,
        is_option: option.is_some(),
        option_type: option.map(|details| details.option_type.clone()),
        strike_price: option.map(|details| details.strike_price),
        expiration_date: option.map(|details| details.expiration_date.clone()),
        raw_json,
    })
}

#[cfg(test)]
mod watermark_tests {
    use super::should_advance_watermark;

    /// Regression: re-registration reported a watermark during an empty backfill.
    #[test]
    fn empty_fetch_against_populated_account_does_not_advance() {
        assert!(!should_advance_watermark(0, 1120));
    }

    #[test]
    fn empty_fetch_against_empty_account_advances() {
        assert!(should_advance_watermark(0, 0));
    }

    #[test]
    fn a_fetch_that_returned_rows_always_advances() {
        assert!(should_advance_watermark(1120, 1120));
        assert!(should_advance_watermark(5, 0));
    }
}
